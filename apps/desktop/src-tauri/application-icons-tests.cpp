#define wmain icon_helper_entry
#include "application-icons.cpp"
#undef wmain
#include <cassert>

struct Fixture {
    std::wstring folder, registry;
    HKEY key = nullptr;
    std::vector<std::wstring> files;
    Fixture() {
        wchar_t temp[MAX_PATH] = {};
        assert(GetTempPathW(MAX_PATH, temp));
        const auto id = std::to_wstring(GetCurrentProcessId()) + L"-" + std::to_wstring(GetTickCount64());
        folder = std::wstring(temp) + L"CaskletIconTest-" + id;
        assert(CreateDirectoryW(folder.c_str(), nullptr));
        registry = L"Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\CaskletIconTest-" + id;
        assert(RegCreateKeyExW(HKEY_CURRENT_USER, registry.c_str(), 0, nullptr, 0, KEY_ALL_ACCESS | KEY_WOW64_64KEY, nullptr, &key, nullptr) == ERROR_SUCCESS);
    }
    ~Fixture() {
        if (key) RegCloseKey(key);
        RegDeleteKeyExW(HKEY_CURRENT_USER, registry.c_str(), KEY_WOW64_64KEY, 0);
        for (const auto& file : files) DeleteFileW(file.c_str());
        RemoveDirectoryW(folder.c_str());
    }
    void value(const wchar_t* name, const std::wstring& text) const {
        assert(RegSetValueExW(key, name, 0, REG_SZ, reinterpret_cast<const BYTE*>(text.c_str()), static_cast<DWORD>((text.size() + 1) * sizeof(wchar_t))) == ERROR_SUCCESS);
    }
    void executable(const wchar_t* name) {
        wchar_t system[MAX_PATH] = {};
        assert(GetSystemDirectoryW(system, MAX_PATH));
        const auto file = folder + L"\\" + name;
        // Resource fixture only: copied application is never executed.
        assert(CopyFileW((std::wstring(system) + L"\\notepad.exe").c_str(), file.c_str(), TRUE));
        files.push_back(file);
    }
    void tiny_file(int index) {
        const auto file = folder + L"\\entry-" + std::to_wstring(index) + L".txt";
        const auto handle = CreateFileW(file.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, nullptr);
        assert(handle != INVALID_HANDLE_VALUE);
        CloseHandle(handle);
        files.push_back(file);
    }
};

int wmain() {
    Gdiplus::GdiplusStartupInput input;
    ULONG_PTR token = 0;
    assert(Gdiplus::GdiplusStartup(&token, &input, nullptr) == Gdiplus::Ok);
    {
        Fixture fixture;
        const auto source = L"HKCU64\\" + fixture.registry;
        fixture.value(L"InstallLocation", fixture.folder);
        assert(registered_icon(source).empty());
        fixture.executable(L"application.exe");
        assert(!registered_icon(source).empty()); // missing DisplayIcon
        fixture.value(L"InstallLocation", L"");
        fixture.value(L"UninstallString", L"\"" + fixture.folder + L"\\uninstall.exe\" /quiet");
        assert(!registered_icon(source).empty()); // missing installation folder, explicit uninstaller metadata
        fixture.value(L"UninstallString", L"");
        fixture.value(L"InstallLocation", fixture.folder);
        fixture.value(L"DisplayIcon", fixture.folder + L"\\missing.exe");
        assert(!registered_icon(source).empty()); // broken DisplayIcon
        fixture.value(L"DisplayIcon", L"\"" + fixture.folder + L"\\application.exe\",999999");
        assert(!registered_icon(source).empty()); // invalid icon resource index
        fixture.value(L"DisplayIcon", L"");
        fixture.executable(L"second.exe");
        assert(registered_icon(source).empty()); // never choose an ambiguous executable
        DeleteFileW(fixture.files.back().c_str());
        fixture.files.pop_back();
        for (int index = 0; index < 65; ++index) fixture.tiny_file(index);
        assert(registered_icon(source).empty()); // bounded immediate-directory fallback
        assert(extract(L"\\\\server\\share\\app.exe").empty());
        assert(extract(L"relative.exe").empty());
        assert(uninstall_folder(L"cmd.exe /c anything").empty());
        assert(uninstall_folder(L"C:\\Program Files\\App\\uninstall.exe").empty());
        assert(uninstall_folder(L"C:\\Windows\\System32\\msiexec.exe /x {guid}").empty());
    }
    Gdiplus::GdiplusShutdown(token);
    std::cout << "Icon metadata fallback, resource index, ambiguity and bounds passed\n";
    return 0;
}
