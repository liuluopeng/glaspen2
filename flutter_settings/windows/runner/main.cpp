#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <windows.h>

#include "flutter_window.h"
#include "utils.h"

int APIENTRY wWinMain(_In_ HINSTANCE instance, _In_opt_ HINSTANCE prev,
                      _In_ wchar_t *command_line, _In_ int show_command) {
  // Attach to console when present (e.g., 'flutter run') or create a
  // new console when running with a debugger.
  if (!::AttachConsole(ATTACH_PARENT_PROCESS) && ::IsDebuggerPresent()) {
    CreateAndAttachConsole();
  }

  // Initialize COM, so that it is available for use in the library and/or
  // plugins.
  ::CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);

  // 与主进程(glaspen2.exe)共用同一个 AppUserModelID:
  // 任务栏把两个进程的窗口归组为同一个图标。
  if (HMODULE shell32 = ::GetModuleHandleW(L"shell32.dll")) {
    using SetAumidFn = HRESULT(WINAPI *)(PCWSTR);
    auto set_aumid = reinterpret_cast<SetAumidFn>(
        ::GetProcAddress(shell32, "SetCurrentProcessExplicitAppUserModelID"));
    if (set_aumid) {
      set_aumid(L"glaspen2.app");
    }
  }

  flutter::DartProject project(L"data");

  std::vector<std::string> command_line_arguments =
      GetCommandLineArguments();

  project.set_dart_entrypoint_arguments(std::move(command_line_arguments));

  FlutterWindow window(project);
  Win32Window::Point origin(100, 100);
  Win32Window::Size size(520, 850);
  if (!window.Create(L"Glaspen2 Settings", origin, size)) {
    return EXIT_FAILURE;
  }
  window.SetQuitOnClose(true);

  ::MSG msg;
  while (::GetMessage(&msg, nullptr, 0, 0)) {
    ::TranslateMessage(&msg);
    ::DispatchMessage(&msg);
  }

  ::CoUninitialize();
  return EXIT_SUCCESS;
}
