#include "flutter_window.h"

#include <optional>
#include <cwchar>
#include <filesystem>
#include <system_error>
#include <vector>

#include <objidl.h>
#include <shobjidl_core.h>
#include <wincodec.h>

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

std::wstring Utf8ToWide(const std::string& value) {
  if (value.empty()) return {};
  const int length = ::MultiByteToWideChar(
      CP_UTF8, MB_ERR_INVALID_CHARS, value.data(), static_cast<int>(value.size()),
      nullptr, 0);
  if (length <= 0) return {};
  std::wstring result(static_cast<size_t>(length), L'\0');
  if (::MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value.data(),
                            static_cast<int>(value.size()), result.data(), length) <= 0) {
    return {};
  }
  return result;
}

std::vector<uint8_t> EncodeJpegThumbnail(HBITMAP bitmap) {
  IWICImagingFactory* factory = nullptr;
  IWICBitmap* source = nullptr;
  IStream* stream = nullptr;
  IWICBitmapEncoder* encoder = nullptr;
  IWICBitmapFrameEncode* frame = nullptr;
  IPropertyBag2* properties = nullptr;
  std::vector<uint8_t> bytes;

  if (FAILED(::CoCreateInstance(CLSID_WICImagingFactory, nullptr, CLSCTX_INPROC_SERVER,
                                IID_PPV_ARGS(&factory))) ||
      FAILED(factory->CreateBitmapFromHBITMAP(bitmap, nullptr,
                                              WICBitmapUsePremultipliedAlpha, &source)) ||
      FAILED(::CreateStreamOnHGlobal(nullptr, TRUE, &stream)) ||
      FAILED(factory->CreateEncoder(GUID_ContainerFormatJpeg, nullptr, &encoder)) ||
      FAILED(encoder->Initialize(stream, WICBitmapEncoderNoCache)) ||
      FAILED(encoder->CreateNewFrame(&frame, &properties))) {
    goto cleanup;
  }
  if (properties != nullptr) {
    PROPBAG2 option = {};
    option.pstrName = const_cast<LPOLESTR>(L"ImageQuality");
    VARIANT quality;
    ::VariantInit(&quality);
    quality.vt = VT_R4;
    quality.fltVal = 0.62F;
    const HRESULT property_result = properties->Write(1, &option, &quality);
    ::VariantClear(&quality);
    if (FAILED(property_result)) goto cleanup;
  }
  if (FAILED(frame->Initialize(properties))) goto cleanup;
  {
    UINT width = 0;
    UINT height = 0;
    GUID pixel_format = GUID_WICPixelFormat24bppBGR;
    if (FAILED(source->GetSize(&width, &height)) || FAILED(frame->SetSize(width, height)) ||
        FAILED(frame->SetPixelFormat(&pixel_format)) || FAILED(frame->WriteSource(source, nullptr)) ||
        FAILED(frame->Commit()) || FAILED(encoder->Commit())) {
      goto cleanup;
    }
  }
  {
    HGLOBAL memory = nullptr;
    if (FAILED(::GetHGlobalFromStream(stream, &memory)) || memory == nullptr) goto cleanup;
    const SIZE_T length = ::GlobalSize(memory);
    const auto* data = static_cast<const uint8_t*>(::GlobalLock(memory));
    if (data == nullptr || length == 0 || length > 24U * 1024U) {
      if (data != nullptr) ::GlobalUnlock(memory);
      goto cleanup;
    }
    bytes.assign(data, data + length);
    ::GlobalUnlock(memory);
  }

cleanup:
  if (properties != nullptr) properties->Release();
  if (frame != nullptr) frame->Release();
  if (encoder != nullptr) encoder->Release();
  if (stream != nullptr) stream->Release();
  if (source != nullptr) source->Release();
  if (factory != nullptr) factory->Release();
  return bytes;
}

std::vector<uint8_t> VideoThumbnail(const std::string& source_path) {
  const std::wstring path = Utf8ToWide(source_path);
  if (path.empty()) return {};
  const HRESULT initialized = ::CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
  const bool should_uninitialize = SUCCEEDED(initialized);
  IShellItemImageFactory* image_factory = nullptr;
  HBITMAP bitmap = nullptr;
  std::vector<uint8_t> preview;
  const HRESULT item_result = ::SHCreateItemFromParsingName(
      path.c_str(), nullptr, IID_PPV_ARGS(&image_factory));
  if (SUCCEEDED(item_result) && image_factory != nullptr &&
      SUCCEEDED(image_factory->GetImage(SIZE{320, 320},
                                        SIIGBF_THUMBNAILONLY | SIIGBF_BIGGERSIZEOK,
                                        &bitmap)) &&
      bitmap != nullptr) {
    preview = EncodeJpegThumbnail(bitmap);
  }
  if (bitmap != nullptr) ::DeleteObject(bitmap);
  if (image_factory != nullptr) image_factory->Release();
  if (should_uninitialize) ::CoUninitialize();
  return preview;
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
  media_channel_ = std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
      flutter_controller_->engine()->messenger(), "torca/media",
      &flutter::StandardMethodCodec::GetInstance());
  media_channel_->SetMethodCallHandler(
      [](const auto& call, auto result) {
        if (call.method_name() != "videoThumbnail") {
          result->NotImplemented();
          return;
        }
        const auto* arguments = std::get_if<flutter::EncodableMap>(call.arguments());
        if (arguments == nullptr) {
          result->Success();
          return;
        }
        const auto iterator = arguments->find(flutter::EncodableValue("sourcePath"));
        if (iterator == arguments->end()) {
          result->Success();
          return;
        }
        const auto* source_path = std::get_if<std::string>(&iterator->second);
        if (source_path == nullptr) {
          result->Success();
          return;
        }
        const std::vector<uint8_t> thumbnail = VideoThumbnail(*source_path);
        if (thumbnail.empty()) {
          result->Success();
        } else {
          result->Success(flutter::EncodableValue(thumbnail));
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
    media_channel_ = nullptr;
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
