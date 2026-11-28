// Native Explorer adapter only. Never opens SQLite, inspects installers, or changes resources.
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <shlobj.h>
#include <shellapi.h>
#include <shlwapi.h>
#include <strsafe.h>
#include <wrl/client.h>
#include <atomic>
#include <string>
#include <vector>
#include <new>

using Microsoft::WRL::ComPtr;
#include "explorer-class-id.h"
static const CLSID ClassId = CASKLET_EXPLORER_CLASS_ID;
static std::atomic<long> objects{0};
static HINSTANCE module;

static std::wstring application_path() {
    std::wstring path(32768, L'\0');
    DWORD length = GetModuleFileNameW(module, path.data(), static_cast<DWORD>(path.size()));
    if (!length || length >= path.size()) return {};
    path.resize(length);
    auto slash = path.find_last_of(L'\\');
    if (slash == std::wstring::npos) return {};
    path.resize(slash);
    slash = path.find_last_of(L'\\');
    if (slash == std::wstring::npos) return {};
    return path.substr(0, slash + 1) + L"casklet-desktop.exe";
}

// Windows CRT command-line encoding. No command interpreter is involved.
static std::wstring quote(const std::wstring& value) {
    std::wstring result = L"\"";
    size_t slashes = 0;
    for (wchar_t c : value) {
        if (c == L'\\') { ++slashes; continue; }
        result.append(slashes * (c == L'\"' ? 2 : 1), L'\\');
        slashes = 0;
        if (c == L'\"') result += L'\\';
        result += c;
    }
    result.append(slashes * 2, L'\\');
    return result + L'\"';
}

struct Destination { std::wstring title; std::wstring id; };
struct Action { std::wstring id; std::wstring verb; };
static bool installer_selection(const std::vector<std::wstring>& paths) {
    if (paths.size() != 1) return false;
    DWORD attributes = GetFileAttributesW(paths[0].c_str());
    if (attributes == INVALID_FILE_ATTRIBUTES || (attributes & FILE_ATTRIBUTE_DIRECTORY)) return false;
    const wchar_t* extension = PathFindExtensionW(paths[0].c_str());
    return _wcsicmp(extension, L".exe") == 0 || _wcsicmp(extension, L".msi") == 0;
}

class Menu final : public IShellExtInit, public IContextMenu {
    std::atomic<ULONG> refs{1};
    std::vector<std::wstring> paths;
    std::vector<Destination> destinations;
    std::vector<Action> actions;
    HBITMAP bitmap = nullptr;
    bool selection_too_large = false;
public:
    Menu() { ++objects; }
    ~Menu() { if (bitmap) DeleteObject(bitmap); --objects; }
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** out) override {
        if (!out) return E_POINTER;
        *out = nullptr;
        if (iid == IID_IUnknown || iid == IID_IShellExtInit) *out = static_cast<IShellExtInit*>(this);
        else if (iid == IID_IContextMenu) *out = static_cast<IContextMenu*>(this);
        else return E_NOINTERFACE;
        AddRef(); return S_OK;
    }
    ULONG STDMETHODCALLTYPE AddRef() override { return ++refs; }
    ULONG STDMETHODCALLTYPE Release() override { ULONG n = --refs; if (!n) delete this; return n; }
    HRESULT STDMETHODCALLTYPE Initialize(PCIDLIST_ABSOLUTE, IDataObject* data, HKEY) override {
        try {
            paths.clear();
            destinations.clear();
            actions.clear();
            selection_too_large = false;
            if (!data) return E_INVALIDARG;
            FORMATETC format{CF_HDROP, nullptr, DVASPECT_CONTENT, -1, TYMED_HGLOBAL};
            STGMEDIUM medium{};
            HRESULT hr = data->GetData(&format, &medium);
            if (FAILED(hr)) return hr;
            struct ReleaseMedium { STGMEDIUM* value; ~ReleaseMedium() { ReleaseStgMedium(value); } } release{&medium};
            if (medium.tymed != TYMED_HGLOBAL || !medium.hGlobal) return DV_E_TYMED;
            const HDROP drop = static_cast<HDROP>(medium.hGlobal);
            UINT count = DragQueryFileW(drop, 0xffffffff, nullptr, 0);
            if (!count) return E_INVALIDARG;
            if (count > 32) { selection_too_large = true; return S_OK; }
            for (UINT i = 0; i < count; ++i) {
                UINT length = DragQueryFileW(drop, i, nullptr, 0);
                if (!length || length > 32760) { paths.clear(); return E_INVALIDARG; }
                std::wstring path(length + 1, L'\0');
                if (DragQueryFileW(drop, i, path.data(), length + 1) != length) { paths.clear(); return E_FAIL; }
                path.resize(length);
                paths.push_back(std::move(path));
            }
            return S_OK;
        } catch (...) { paths.clear(); return E_OUTOFMEMORY; }
    }
    HRESULT STDMETHODCALLTYPE QueryContextMenu(HMENU menu, UINT position, UINT first, UINT last, UINT flags) override {
        if (flags & CMF_DEFAULTONLY) return MAKE_HRESULT(SEVERITY_SUCCESS, 0, 0);
        if (selection_too_large) {
            if (!InsertMenuW(menu, position, MF_BYPOSITION | MF_STRING | MF_GRAYED, first, L"Casklet (select up to 32 items)")) return E_FAIL;
            return MAKE_HRESULT(SEVERITY_SUCCESS, 0, 1);
        }
        if (paths.empty() || last < first) return MAKE_HRESULT(SEVERITY_SUCCESS, 0, 0);
        try {
            destinations.clear();
            actions.clear();
            std::vector<wchar_t> snapshot(8192, L'\0');
            DWORD bytes = static_cast<DWORD>(snapshot.size() * sizeof(wchar_t));
            if (RegGetValueW(HKEY_CURRENT_USER, L"Software\\Casklet\\Explorer", L"Snapshot", RRF_RT_REG_MULTI_SZ, nullptr, snapshot.data(), &bytes) == ERROR_SUCCESS && bytes >= 4) {
                snapshot.resize(bytes / sizeof(wchar_t));
                std::vector<std::wstring> fields;
                size_t offset = 0;
                while (offset < snapshot.size() && snapshot[offset] && fields.size() <= 12) {
                    size_t end = offset;
                    while (end < snapshot.size() && snapshot[end]) ++end;
                    if (end == snapshot.size()) { fields.clear(); break; }
                    fields.emplace_back(snapshot.data() + offset, end - offset);
                    offset = end + 1;
                }
                if (fields.size() <= 12 && fields.size() % 2 == 0) {
                    for (size_t i = 0; i < fields.size(); i += 2) destinations.push_back({std::move(fields[i]), std::move(fields[i+1])});
                }
            }
            if (destinations.empty()) destinations.push_back({L"Choose a cask in Casklet\u2026", L"choose"});
            const bool installer = installer_selection(paths);
            size_t needed = 0;
            for (const auto& destination : destinations) needed += destination.id == L"choose" ? 1 : installer ? 3 : 2;
            if (needed > static_cast<size_t>(last - first) + 1) return MAKE_HRESULT(SEVERITY_SUCCESS, 0, 0);
            HMENU popup = CreatePopupMenu();
            if (!popup) return HRESULT_FROM_WIN32(GetLastError());
            for (const auto& destination : destinations) {
                if (destination.id == L"choose") {
                    if (!AppendMenuW(popup, MF_STRING, first + actions.size(), destination.title.c_str())) { DestroyMenu(popup); return E_FAIL; }
                    actions.push_back({destination.id, L"--explorer"});
                } else {
                    HMENU nested = CreatePopupMenu();
                    if (!nested) { DestroyMenu(popup); return E_FAIL; }
                    auto add = [&](const wchar_t* title, const wchar_t* verb) {
                        if (!AppendMenuW(nested, MF_STRING, first + actions.size(), title)) return false;
                        actions.push_back({destination.id, verb}); return true;
                    };
                    if ((installer && !add(L"Install Application", L"--explorer-install")) ||
                        !add(L"Add as Owned", L"--explorer-owned") || !add(L"Add as Reference", L"--explorer-reference") ||
                        !AppendMenuW(popup, MF_POPUP, reinterpret_cast<UINT_PTR>(nested), destination.title.c_str())) {
                        DestroyMenu(nested); DestroyMenu(popup); return E_FAIL;
                    }
                }
            }
            MENUITEMINFOW entry{sizeof(entry)};
            entry.fMask = MIIM_STRING | MIIM_SUBMENU;
            entry.dwTypeData = const_cast<wchar_t*>(L"Casklet");
            entry.hSubMenu = popup;
            // The DLL contains the same packaged app icon. Render a menu-sized DIB.
            if (!bitmap) {
                int size = GetSystemMetrics(SM_CXSMICON);
                HICON icon = nullptr;
                auto executable = application_path();
                ExtractIconExW(executable.c_str(), 0, nullptr, &icon, 1);
                if (icon) {
                    HDC dc = CreateCompatibleDC(nullptr);
                    BITMAPINFO info{}; info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER); info.bmiHeader.biWidth = size; info.bmiHeader.biHeight = -size;
                    info.bmiHeader.biPlanes = 1; info.bmiHeader.biBitCount = 32; info.bmiHeader.biCompression = BI_RGB;
                    void* pixels = nullptr;
                    bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &pixels, nullptr, 0);
                    if (bitmap) {
                        ZeroMemory(pixels, size * size * 4);
                        HGDIOBJ old = SelectObject(dc, bitmap); DrawIconEx(dc, 0, 0, icon, size, size, 0, nullptr, DI_NORMAL); SelectObject(dc, old);
                    }
                    DeleteDC(dc); DestroyIcon(icon);
                }
            }
            if (bitmap) { entry.fMask |= MIIM_BITMAP; entry.hbmpItem = bitmap; }
            if (!InsertMenuItemW(menu, position, TRUE, &entry)) { DestroyMenu(popup); return E_FAIL; }
            return MAKE_HRESULT(SEVERITY_SUCCESS, 0, static_cast<USHORT>(actions.size()));
        } catch (...) { return E_OUTOFMEMORY; }
    }
    HRESULT STDMETHODCALLTYPE InvokeCommand(CMINVOKECOMMANDINFO* info) override {
        if (!info || info->cbSize < sizeof(CMINVOKECOMMANDINFO) || HIWORD(info->lpVerb)) return E_INVALIDARG;
        try {
            size_t index = LOWORD(info->lpVerb);
            if (index >= actions.size()) return E_INVALIDARG;
            auto executable = application_path();
            if (executable.empty()) return HRESULT_FROM_WIN32(ERROR_FILE_NOT_FOUND);
            const auto& id = actions[index].id;
            if (id.empty() || id.size() > 128 || id.find_first_not_of(L"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-") != std::wstring::npos) return E_INVALIDARG;
            std::wstring command = quote(executable) + L" " + actions[index].verb + L" " + id + L" --";
            for (const auto& path : paths) command += L" " + quote(path);
            if (command.size() >= 32767) {
                MessageBoxW(info->hwnd, L"The selected paths are too long for one launch. Select fewer items and try again.", L"Casklet", MB_OK | MB_ICONERROR);
                return HRESULT_FROM_WIN32(ERROR_FILENAME_EXCED_RANGE);
            }
            STARTUPINFOW startup{sizeof(startup)};
            PROCESS_INFORMATION process{};
            if (!CreateProcessW(executable.c_str(), command.data(), nullptr, nullptr, FALSE, 0, nullptr, nullptr, &startup, &process)) {
                DWORD error = GetLastError();
                MessageBoxW(info->hwnd, L"Casklet could not be opened. Reinstall Casklet to repair Explorer integration, then try again.", L"Casklet", MB_OK | MB_ICONERROR);
                return HRESULT_FROM_WIN32(error);
            }
            CloseHandle(process.hThread); CloseHandle(process.hProcess);
            return S_OK;
        } catch (...) { return E_OUTOFMEMORY; }
    }
    HRESULT STDMETHODCALLTYPE GetCommandString(UINT_PTR index, UINT flags, UINT*, LPSTR text, UINT count) override {
        if (index >= actions.size()) return E_INVALIDARG;
        if (flags == GCS_HELPTEXTW) return StringCchCopyW(reinterpret_cast<LPWSTR>(text), count, actions[index].verb == L"--explorer-owned" ? L"Grant this Cask ownership of dedicated data at its current location. Cleanup still requires an approved plan." : actions[index].verb == L"--explorer-reference" ? L"Add a reference at its current location. Reference removal keeps the data." : L"Open the existing Casklet review workflow.");
        if (flags == GCS_VALIDATEW || flags == GCS_VALIDATEA) return S_OK;
        return E_NOTIMPL;
    }
};

class Factory final : public IClassFactory {
    std::atomic<ULONG> refs{1};
public:
    Factory() { ++objects; }
    ~Factory() { --objects; }
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** out) override {
        if (!out) return E_POINTER; *out = nullptr;
        if (iid != IID_IUnknown && iid != IID_IClassFactory) return E_NOINTERFACE;
        *out = static_cast<IClassFactory*>(this); AddRef(); return S_OK;
    }
    ULONG STDMETHODCALLTYPE AddRef() override { return ++refs; }
    ULONG STDMETHODCALLTYPE Release() override { ULONG n = --refs; if (!n) delete this; return n; }
    HRESULT STDMETHODCALLTYPE CreateInstance(IUnknown* outer, REFIID iid, void** out) override {
        if (!out) return E_POINTER; *out = nullptr;
        if (outer) return CLASS_E_NOAGGREGATION;
        auto menu = new (std::nothrow) Menu(); if (!menu) return E_OUTOFMEMORY;
        HRESULT result = menu->QueryInterface(iid, out); menu->Release(); return result;
    }
    HRESULT STDMETHODCALLTYPE LockServer(BOOL lock) override { objects += lock ? 1 : -1; return S_OK; }
};

extern "C" BOOL WINAPI DllMain(HINSTANCE instance, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) module = instance;
    return TRUE;
}
STDAPI DllCanUnloadNow() { return objects == 0 ? S_OK : S_FALSE; }
STDAPI DllGetClassObject(REFCLSID clsid, REFIID iid, void** out) {
    if (!out) return E_POINTER; *out = nullptr;
    if (clsid != ClassId) return CLASS_E_CLASSNOTAVAILABLE;
    auto factory = new (std::nothrow) Factory(); if (!factory) return E_OUTOFMEMORY;
    HRESULT result = factory->QueryInterface(iid, out); factory->Release(); return result;
}
