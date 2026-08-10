#include "flutter_window.h"

#include <optional>
#include <cwchar>
#include <filesystem>
#include <system_error>

#include "flutter/generated_plugin_registrant.h"

namespace {
bool ResetRuntimeData() {
  wchar_t buffer[MAX_PATH] = {};
  const DWORD length = ::GetEnvironmentVariableW(L"LOCALAPPDATA", buffer, MAX_PATH);
  if (length == 0 || length >= MAX_PATH) return false;
  const std::filesystem::path root = std::filesystem::path(buffer) / L"Torca";
  if (!std::filesystem::exists(root)) return true;
  SYSTEMTIME now = {};
  ::GetSystemTime(&now);
  wchar_t backup_name[96] = {};
  ::swprintf_s(
      backup_name, L"reset-%04u%02u%02u-%02u%02u%02u-%lu", now.wYear,
      now.wMonth, now.wDay, now.wHour, now.wMinute, now.wSecond,
      ::GetCurrentProcessId());
  const std::filesystem::path backup_root =
      std::filesystem::path(buffer) / L"Torca-backups";
  const std::filesystem::path backup = backup_root / backup_name;
  std::error_code error;
  std::filesystem::create_directories(backup_root, error);
  if (error) return false;
  std::filesystem::rename(root, backup, error);
  if (error) return false;
  std::filesystem::create_directories(root, error);
  return !error;
}
}  // namespace

FlutterWindow::FlutterWindow(const flutter::DartProject& project)
    : project_(project) {}

FlutterWindow::~FlutterWindow() {}

bool FlutterWindow::OnCreate() {
  if (!Win32Window::OnCreate()) {
    return false;
  }

  RECT frame = GetClientArea();

  // The size here must match the window dimensions to avoid unnecessary surface
  // creation / destruction in the startup path.
  flutter_controller_ = std::make_unique<flutter::FlutterViewController>(
      frame.right - frame.left, frame.bottom - frame.top, project_);
  // Ensure that basic setup of the controller was successful.
  if (!flutter_controller_->engine() || !flutter_controller_->view()) {
    return false;
  }
  RegisterPlugins(flutter_controller_->engine());
  runtime_channel_ = std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
      flutter_controller_->engine()->messenger(), "torca/runtime",
      &flutter::StandardMethodCodec::GetInstance());
  runtime_channel_->SetMethodCallHandler(
      [](const auto& call, auto result) {
        if (call.method_name() == "resetRuntime") {
          result->Success(flutter::EncodableValue(ResetRuntimeData()));
        } else {
          result->NotImplemented();
        }
      });
  SetChildContent(flutter_controller_->view()->GetNativeWindow());

  flutter_controller_->engine()->SetNextFrameCallback([&]() {
    this->Show();
  });

  // Flutter can complete the first frame before the "show window" callback is
  // registered. The following call ensures a frame is pending to ensure the
  // window is shown. It is a no-op if the first frame hasn't completed yet.
  flutter_controller_->ForceRedraw();

  return true;
}

void FlutterWindow::OnDestroy() {
  if (flutter_controller_) {
    runtime_channel_ = nullptr;
    flutter_controller_ = nullptr;
  }

  Win32Window::OnDestroy();
}

LRESULT
FlutterWindow::MessageHandler(HWND hwnd, UINT const message,
                              WPARAM const wparam,
                              LPARAM const lparam) noexcept {
  // Give Flutter, including plugins, an opportunity to handle window messages.
  if (flutter_controller_) {
    std::optional<LRESULT> result =
        flutter_controller_->HandleTopLevelWindowProc(hwnd, message, wparam,
                                                      lparam);
    if (result) {
      return *result;
    }
  }

  switch (message) {
    case WM_FONTCHANGE:
      flutter_controller_->engine()->ReloadSystemFonts();
      break;
  }

  return Win32Window::MessageHandler(hwnd, message, wparam, lparam);
}
