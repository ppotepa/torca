#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <windows.h>

#include "flutter_window.h"
#include "utils.h"

namespace {
constexpr wchar_t kTorcaMutex[] = L"Local\\Torca-0.1-SingleInstance";
constexpr wchar_t kTorcaWindowTitle[] = L"Torca";

BOOL CALLBACK ActivateExistingWindow(HWND window, LPARAM) {
  wchar_t title[256] = {};
  if (::GetWindowTextW(window, title, static_cast<int>(std::size(title))) <= 0) {
    return TRUE;
  }
  if (::wcscmp(title, kTorcaWindowTitle) != 0) {
    return TRUE;
  }
  ::ShowWindow(window, SW_RESTORE);
  ::SetForegroundWindow(window);
  return FALSE;
}
}  // namespace

int APIENTRY wWinMain(_In_ HINSTANCE instance, _In_opt_ HINSTANCE prev,
                      _In_ wchar_t* command_line, _In_ int show_command) {
  HANDLE instance_mutex = ::CreateMutexW(nullptr, TRUE, kTorcaMutex);
  if (instance_mutex == nullptr) {
    return EXIT_FAILURE;
  }
  if (::GetLastError() == ERROR_ALREADY_EXISTS) {
    ::EnumWindows(ActivateExistingWindow, 0);
    ::CloseHandle(instance_mutex);
    return EXIT_SUCCESS;
  }

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
