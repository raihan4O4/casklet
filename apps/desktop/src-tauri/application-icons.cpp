// Read icon resources as data in a short-lived process, never load application code.
#include <windows.h>
#include <shellapi.h>
#include <shlwapi.h>
#include <gdiplus.h>
#include <wincrypt.h>
#include <msi.h>
#include <algorithm>
#include <iostream>
#include <string>
#include <vector>

static std::string extract(std::wstring path);

static std::wstring registry_string(HKEY key, const wchar_t* name) {
    wchar_t value[32768] = {};
    DWORD bytes = sizeof(value), type = 0;
    const auto result = RegQueryValueExW(key, name, nullptr, &type,
        reinterpret_cast<BYTE*>(value), &bytes);
    if (result != ERROR_SUCCESS || (type != REG_SZ && type != REG_EXPAND_SZ) || bytes >= sizeof(value)) return {};
    return value;
}

static std::wstring local_path(std::wstring path) {
    wchar_t expanded[32768] = {};
    const auto count = ExpandEnvironmentStringsW(path.c_str(), expanded, 32768);
    if (!count || count > 32768) return {};
    path = expanded;
    PathUnquoteSpacesW(path.data());
    path.resize(wcslen(path.c_str()));
    if (path.size() < 3 || !iswalpha(path[0]) || path[1] != L':' || path[2] != L'\\') return {};
    const auto drive = path.substr(0, 3);
    const auto type = GetDriveTypeW(drive.c_str());
    if (type != DRIVE_FIXED && type != DRIVE_REMOVABLE && type != DRIVE_RAMDISK) return {};
    return path;
}

// Inspect at most 64 immediate entries, never descend or choose among ambiguous executables.
static std::string unique_application_icon(std::wstring folder) {
    folder = local_path(folder);
    while (!folder.empty() && folder.back() == L'\\') folder.pop_back();
    if (folder.size() < 4) return {};
    WIN32_FIND_DATAW entry = {};
    const auto search = FindFirstFileW((folder + L"\\*").c_str(), &entry);
    if (search == INVALID_HANDLE_VALUE) return {};
    std::wstring candidate;
    size_t entries = 0;
    bool ambiguous = false;
    do {
        if (++entries > 64) { ambiguous = true; break; }
        if (entry.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT)) continue;
        std::wstring name(entry.cFileName);
        std::transform(name.begin(), name.end(), name.begin(), [](wchar_t c) { return static_cast<wchar_t>(towlower(c)); });
        if (_wcsicmp(PathFindExtensionW(name.c_str()), L".exe")) continue;
        if (name.find(L"unins") == 0 || name.find(L"setup") == 0 || name.find(L"update") == 0 || name.find(L"crash") == 0 || name.find(L"helper") != std::wstring::npos) continue;
        if (!candidate.empty()) { ambiguous = true; break; }
        candidate = folder + L"\\" + entry.cFileName;
    } while (FindNextFileW(search, &entry));
    FindClose(search);
    return ambiguous || candidate.empty() ? std::string{} : extract(candidate);
}

// Use only a syntactically explicit local executable path as folder metadata.
// Never invoke the uninstall command or treat its own icon as the application's icon.
static std::wstring uninstall_folder(std::wstring command) {
    if (command.empty()) return {};
    if (command.front() == L'"') {
        const auto closing = command.find(L'"', 1);
        if (closing == std::wstring::npos) return {};
        command = command.substr(1, closing - 1);
    } else {
        const auto whitespace = command.find_first_of(L" \t\r\n");
        if (whitespace != std::wstring::npos) command.resize(whitespace);
    }
    command = local_path(command);
    if (command.empty() || _wcsicmp(PathFindExtensionW(command.c_str()), L".exe")) return {};
    // Also reject an ambiguous unquoted path such as C:\Program.exe Files\... .
    const auto file = PathFindFileNameW(command.c_str());
    if (_wcsicmp(file, L"uninstall.exe") && _wcsnicmp(file, L"unins", 5)) return {};
    if (!PathRemoveFileSpecW(command.data())) return {};
    command.resize(wcslen(command.c_str()));
    return command;
}

static std::string registered_icon(const std::wstring& source) {
    const auto separator = source.find(L'\\');
    if (separator == std::wstring::npos) return {};
    const auto view = source.substr(0, separator);
    const auto path = source.substr(separator + 1);
    const std::wstring prefix = L"Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\";
    if (_wcsnicmp(path.c_str(), prefix.c_str(), prefix.size()) != 0 || path.size() <= prefix.size()) return {};
    HKEY hive = nullptr;
    REGSAM flags = KEY_QUERY_VALUE;
    if (view == L"HKCU64" || view == L"HKCU32") hive = HKEY_CURRENT_USER;
    if (view == L"HKLM64" || view == L"HKLM32") hive = HKEY_LOCAL_MACHINE;
    if (!hive) return {};
    flags |= view.substr(4) == L"64" ? KEY_WOW64_64KEY : KEY_WOW64_32KEY;
    HKEY key = nullptr;
    if (RegOpenKeyExW(hive, path.c_str(), 0, flags, &key) != ERROR_SUCCESS) return {};
    const auto display_icon = registry_string(key, L"DisplayIcon");
    const auto folder = registry_string(key, L"InstallLocation");
    const auto uninstaller = registry_string(key, L"UninstallString");
    DWORD windows_installer = 0, bytes = sizeof(windows_installer), type = 0;
    const bool msi = RegQueryValueExW(key, L"WindowsInstaller", nullptr, &type,
        reinterpret_cast<BYTE*>(&windows_installer), &bytes) == ERROR_SUCCESS && type == REG_DWORD && windows_installer == 1;
    RegCloseKey(key);
    auto icon = extract(display_icon);
    if (!icon.empty()) return icon;
    const auto product = path.substr(path.find_last_of(L'\\') + 1);
    if (msi && product.size() == 38 && product.front() == L'{' && product.back() == L'}') {
        wchar_t value[32768] = {};
        DWORD length = 32768;
        if (MsiGetProductInfoW(product.c_str(), INSTALLPROPERTY_PRODUCTICON, value, &length) == ERROR_SUCCESS) {
            icon = extract(value);
            if (!icon.empty()) return icon;
        }
    }
    icon = unique_application_icon(folder);
    return icon.empty() ? unique_application_icon(uninstall_folder(uninstaller)) : icon;
}

static std::string extract(std::wstring path) {
    wchar_t expanded[32768] = {};
    const auto count = ExpandEnvironmentStringsW(path.c_str(), expanded, 32768);
    if (!count || count > 32768) return {};
    path = expanded;
    if (!path.empty() && path.front() == L'@') path.erase(0, 1);
    // Windows parses the optional icon index as data; no command line is executed.
    const int index = PathParseIconLocationW(path.data());
    path.resize(wcslen(path.c_str()));
    PathUnquoteSpacesW(path.data());
    path.resize(wcslen(path.c_str()));
    // No network shares, device namespaces, relative paths, or directory scanning.
    path = local_path(path);
    if (path.empty()) return {};
    const auto extension = PathFindExtensionW(path.c_str());
    if (_wcsicmp(extension, L".exe") && _wcsicmp(extension, L".dll") && _wcsicmp(extension, L".ico")) return {};
    HICON icon = nullptr;
    ExtractIconExW(path.c_str(), index, &icon, nullptr, 1);
    if (!icon && index != 0) ExtractIconExW(path.c_str(), 0, &icon, nullptr, 1);
    if (!icon) return {};
    Gdiplus::Bitmap bitmap(icon);
    DestroyIcon(icon);
    IStream* stream = nullptr;
    if (CreateStreamOnHGlobal(nullptr, TRUE, &stream) != S_OK) return {};
    const CLSID png = {0x557cf406, 0x1a04, 0x11d3, {0x9a,0x73,0x00,0x00,0xf8,0x1e,0xf3,0x2e}};
    std::string result;
    if (bitmap.Save(stream, &png, nullptr) == Gdiplus::Ok) {
        STATSTG stat = {};
        if (stream->Stat(&stat, STATFLAG_NONAME) == S_OK && stat.cbSize.QuadPart <= 65536) {
            LARGE_INTEGER start = {};
            stream->Seek(start, STREAM_SEEK_SET, nullptr);
            std::vector<BYTE> bytes(static_cast<size_t>(stat.cbSize.QuadPart));
            ULONG read = 0;
            if (stream->Read(bytes.data(), static_cast<ULONG>(bytes.size()), &read) == S_OK) {
                DWORD length = 0;
                CryptBinaryToStringA(bytes.data(), read, CRYPT_STRING_BASE64 | CRYPT_STRING_NOCRLF, nullptr, &length);
                result.resize(length);
                if (CryptBinaryToStringA(bytes.data(), read, CRYPT_STRING_BASE64 | CRYPT_STRING_NOCRLF, result.data(), &length)) result.resize(length);
                else result.clear();
            }
        }
    }
    stream->Release();
    return result;
}

int wmain(int argc, wchar_t** argv) {
    if (argc > 17) return 1;
    Gdiplus::GdiplusStartupInput input;
    ULONG_PTR token = 0;
    if (Gdiplus::GdiplusStartup(&token, &input, nullptr) != Gdiplus::Ok) return 1;
    for (int i = 1; i < argc; ++i) {
        const std::wstring value(argv[i]);
        std::cout << (value.compare(0, 9, L"registry:") == 0
            ? registered_icon(value.substr(9)) : extract(value)) << '\n';
    }
    Gdiplus::GdiplusShutdown(token);
    return 0;
}
