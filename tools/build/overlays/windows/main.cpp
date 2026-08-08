#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <windows.h>

#include <filesystem>
#include <fstream>
#include <string>

#include "flutter_window.h"
#include "utils.h"

#ifndef WDA_EXCLUDEFROMCAPTURE
#define WDA_EXCLUDEFROMCAPTURE 0x00000011
#endif

namespace {
constexpr wchar_t kTorcaMutex[] = L"Local\\Torca-0.1-SingleInstance";
constexpr wchar_t kTorcaWindowTitle[] = L"Torca";
constexpr wchar_t kTorcaSchemeKey[] = L"Software\\Classes\\torca";
constexpr wchar_t kPairPrefix[] = L"torca://pair?";

BOOL CALLBACK ActivateExistingWindow(HWND window, LPARAM) {
  wchar_t title[256] = {};
  if (::GetWindowTextW(window, title, static_cast<int>(std::size(title))) <= 0) return TRUE;
  if (::wcscmp(title, kTorcaWindowTitle) != 0) return TRUE;
  ::ShowWindow(window, SW_RESTORE);
  ::SetForegroundWindow(window);
  return FALSE;
}

bool IsPairingLink(const wchar_t* command_line) {
  if (command_line == nullptr) return false;
  std::wstring value(command_line);
  while (!value.empty() && iswspace(value.front())) value.erase(value.begin());
  if (value.size() >= 2 && value.front() == L'"' && value.back() == L'"') {
    value = value.substr(1, value.size() - 2);
  }
  return value.rfind(kPairPrefix, 0) == 0 && value.size() <= 512;
}

std::filesystem::path RuntimeRoot() {
  wchar_t buffer[MAX_PATH] = {};
  const DWORD length = ::GetEnvironmentVariableW(L"LOCALAPPDATA", buffer, MAX_PATH);
  if (length == 0 || length >= MAX_PATH) return {};
  return std::filesystem::path(buffer) / L"Torca" / L"0.1";
}

void WritePendingPairingLink(const wchar_t* command_line) {
  if (!IsPairingLink(command_line)) return;
  const auto root = RuntimeRoot();
  if (root.empty()) return;
  std::error_code error;
  std::filesystem::create_directories(root, error);
  if (error) return;
  const auto temporary = root / L"pending_link.tmp";
  const auto destination = root / L"pending_link.txt";
  {
    std::wofstream output(temporary, std::ios::trunc);
    if (!output) return;
    std::wstring value(command_line);
    if (value.size() >= 2 && value.front() == L'"' && value.back() == L'"') {
      value = value.substr(1, value.size() - 2);
    }
    output << value;
    output.flush();
    if (!output) return;
  }
  ::MoveFileExW(temporary.c_str(), destination.c_str(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH);
}

void RegisterTorcaScheme() {
  wchar_t executable[MAX_PATH] = {};
  const DWORD length = ::GetModuleFileNameW(nullptr, executable, MAX_PATH);
  if (length == 0 || length >= MAX_PATH) return;

  HKEY scheme = nullptr;
  if (::RegCreateKeyExW(HKEY_CURRENT_USER, kTorcaSchemeKey, 0, nullptr, 0, KEY_WRITE,
                        nullptr, &scheme, nullptr) != ERROR_SUCCESS) return;
  const wchar_t description[] = L"URL:Torca Pairing Protocol";
  ::RegSetValueExW(scheme, nullptr, 0, REG_SZ,
                   reinterpret_cast<const BYTE*>(description), sizeof(description));
  const wchar_t empty[] = L"";
  ::RegSetValueExW(scheme, L"URL Protocol", 0, REG_SZ,
                   reinterpret_cast<const BYTE*>(empty), sizeof(empty));
  ::RegCloseKey(scheme);

  HKEY command_key = nullptr;
  const std::wstring command_path = std::wstring(kTorcaSchemeKey) + L"\\shell\\open\\command";
  if (::RegCreateKeyExW(HKEY_CURRENT_USER, command_path.c_str(), 0, nullptr, 0, KEY_WRITE,
                        nullptr, &command_key, nullptr) != ERROR_SUCCESS) return;
  const std::wstring command = L"\"" + std::wstring(executable) + L"\" \"%1\"";
  ::RegSetValueExW(command_key, nullptr, 0, REG_SZ,
                   reinterpret_cast<const BYTE*>(command.c_str()),
                   static_cast<DWORD>((command.size() + 1) * sizeof(wchar_t)));
  ::RegCloseKey(command_key);
}

void EnableCaptureProtection(HWND window) {
  if (window == nullptr) return;
  // Exclude Torca from Recall/screen-capture surfaces when the OS supports it. Older Windows
  // versions fall back to WDA_MONITOR, which still protects normal capture APIs.
  if (!::SetWindowDisplayAffinity(window, WDA_EXCLUDEFROMCAPTURE)) {
    ::SetWindowDisplayAffinity(window, WDA_MONITOR);
  }
}
}  // namespace

int APIENTRY wWinMain(_In_ HINSTANCE instance, _In_opt_ HINSTANCE prev,
                      _In_ wchar_t* command_line, _In_ int show_command) {
  HANDLE instance_mutex = ::CreateMutexW(nullptr, TRUE, kTorcaMutex);
  if (instance_mutex == nullptr) return EXIT_FAILURE;
  if (::GetLastError() == ERROR_ALREADY_EXISTS) {
    WritePendingPairingLink(command_line);
    ::EnumWindows(ActivateExistingWindow, 0);
    ::CloseHandle(instance_mutex);
    return EXIT_SUCCESS;
  }

  RegisterTorcaScheme();

  if (!::AttachConsole(ATTACH_PARENT_PROCESS) && ::IsDebuggerPresent()) {
    CreateAndAttachConsole();
  }
  ::CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);

  flutter::DartProject project(L"data");
  std::vector<std::string> command_line_arguments = GetCommandLineArguments();
  project.set_dart_entrypoint_arguments(std::move(command_line_arguments));

  FlutterWindow window(project);
  Win32Window::Point origin(10, 10);
  Win32Window::Size size(1280, 720);
  if (!window.Create(kTorcaWindowTitle, origin, size)) {
    ::CoUninitialize();
    ::ReleaseMutex(instance_mutex);
    ::CloseHandle(instance_mutex);
    return EXIT_FAILURE;
  }
  EnableCaptureProtection(window.GetHandle());
  window.SetQuitOnClose(true);

  ::MSG msg;
  while (::GetMessage(&msg, nullptr, 0, 0)) {
    ::TranslateMessage(&msg);
    ::DispatchMessage(&msg);
  }

  ::CoUninitialize();
  ::ReleaseMutex(instance_mutex);
  ::CloseHandle(instance_mutex);
  return EXIT_SUCCESS;
}
