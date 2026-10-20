// Native ABI/selection/encoding tests; no registry writes, process launches, or file mutations.
#include "extension.cpp"
#include <cassert>
#include <cstring>
#include <iostream>

class RegistryFixture {
    HKEY root = nullptr;
    std::wstring name;
public:
    explicit RegistryFixture(const std::wstring& firstId = L"cask-0") {
        name = L"Software\\CaskletNativeTests\\" + std::to_wstring(GetCurrentProcessId()) + L"-" + std::to_wstring(GetTickCount64());
        assert(RegCreateKeyExW(HKEY_CURRENT_USER, name.c_str(), 0, nullptr, 0, KEY_ALL_ACCESS, nullptr, &root, nullptr) == ERROR_SUCCESS);
        assert(RegOverridePredefKey(HKEY_CURRENT_USER, root) == ERROR_SUCCESS);
        HKEY key = nullptr;
        assert(RegCreateKeyExW(HKEY_CURRENT_USER, L"Software\\Casklet\\Explorer", 0, nullptr, 0, KEY_ALL_ACCESS, nullptr, &key, nullptr) == ERROR_SUCCESS);
        std::vector<wchar_t> data;
        for (int i = 0; i < 6; ++i) {
            for (const auto& value : {i == 5 ? std::wstring(L"More casks…") : L"Project " + std::to_wstring(i), i == 5 ? std::wstring(L"choose") : i == 0 ? firstId : L"cask-" + std::to_wstring(i)}) {
                data.insert(data.end(), value.begin(), value.end()); data.push_back(0);
            }
        }
        data.push_back(0);
        assert(RegSetValueExW(key, L"Snapshot", 0, REG_MULTI_SZ, reinterpret_cast<const BYTE*>(data.data()), static_cast<DWORD>(data.size() * sizeof(wchar_t))) == ERROR_SUCCESS);
        RegCloseKey(key);
    }
    void restore() {
        if (!root) return;
        assert(RegOverridePredefKey(HKEY_CURRENT_USER, nullptr) == ERROR_SUCCESS);
        RegCloseKey(root); root = nullptr;
        assert(RegDeleteTreeW(HKEY_CURRENT_USER, name.c_str()) == ERROR_SUCCESS);
    }
    ~RegistryFixture() { restore(); }
};

class DropData final : public IDataObject {
    std::atomic<ULONG> refs{1};
    std::vector<std::wstring> files;
public:
    explicit DropData(std::vector<std::wstring> value) : files(std::move(value)) {}
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** out) override {
        if (!out) return E_POINTER; *out = nullptr;
        if (iid != IID_IUnknown && iid != IID_IDataObject) return E_NOINTERFACE;
        *out = static_cast<IDataObject*>(this); AddRef(); return S_OK;
    }
    ULONG STDMETHODCALLTYPE AddRef() override { return ++refs; }
    ULONG STDMETHODCALLTYPE Release() override { auto n = --refs; if (!n) delete this; return n; }
    HRESULT STDMETHODCALLTYPE GetData(FORMATETC* format, STGMEDIUM* medium) override {
        if (!format || !medium || format->cfFormat != CF_HDROP) return E_INVALIDARG;
        size_t chars = 1; for (const auto& file : files) chars += file.size() + 1;
        HGLOBAL memory = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, sizeof(DROPFILES) + chars * sizeof(wchar_t));
        if (!memory) return E_OUTOFMEMORY;
        auto drop = static_cast<DROPFILES*>(GlobalLock(memory));
        if (!drop) { GlobalFree(memory); return E_OUTOFMEMORY; }
        drop->pFiles = sizeof(DROPFILES); drop->fWide = TRUE;
        auto next = reinterpret_cast<wchar_t*>(reinterpret_cast<BYTE*>(drop) + sizeof(DROPFILES));
        for (const auto& file : files) { std::memcpy(next, file.c_str(), (file.size() + 1) * sizeof(wchar_t)); next += file.size() + 1; }
        GlobalUnlock(memory);
        medium->tymed = TYMED_HGLOBAL; medium->hGlobal = memory; medium->pUnkForRelease = nullptr;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetDataHere(FORMATETC*, STGMEDIUM*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE QueryGetData(FORMATETC*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE GetCanonicalFormatEtc(FORMATETC*, FORMATETC*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE SetData(FORMATETC*, STGMEDIUM*, BOOL) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE EnumFormatEtc(DWORD, IEnumFORMATETC**) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE DAdvise(FORMATETC*, DWORD, IAdviseSink*, DWORD*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE DUnadvise(DWORD) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE EnumDAdvise(IEnumSTATDATA**) override { return E_NOTIMPL; }
};

int wmain(int arguments, wchar_t** options) {
    if (arguments == 5 && std::wstring(options[1]) == L"--invoke-reference") {
        RegistryFixture fixture(options[3]);
        HMODULE library = LoadLibraryW(options[2]);
        if (!library) return 30;
        auto getFactory = reinterpret_cast<HRESULT(STDAPICALLTYPE*)(REFCLSID, REFIID, void**)>(GetProcAddress(library, "DllGetClassObject"));
        if (!getFactory) return 31;
        ComPtr<IClassFactory> factory;
        if (FAILED(getFactory(ClassId, IID_PPV_ARGS(&factory)))) return 32;
        ComPtr<IShellExtInit> extension;
        if (FAILED(factory->CreateInstance(nullptr, IID_PPV_ARGS(&extension)))) return 33;
        auto data = new DropData({std::wstring(options[4])});
        HRESULT initialized = extension->Initialize(nullptr, data, nullptr); data->Release();
        if (FAILED(initialized)) return 34;
        ComPtr<IContextMenu> context; if (FAILED(extension.As(&context))) return 35;
        HMENU parent = CreatePopupMenu();
        if (FAILED(context->QueryContextMenu(parent, 0, 100, 200, CMF_NORMAL))) return 36;
        HMENU child = GetSubMenu(GetSubMenu(parent, 0), 0);
        int count = GetMenuItemCount(child);
        if (count != 3) return 37;
        wchar_t title[128]{}; GetMenuStringW(child, 2, title, 128, MF_BYPOSITION);
        if (std::wstring(title) != L"Add as Reference") return 38;
        CMINVOKECOMMANDINFO command{sizeof(command)};
        command.lpVerb = MAKEINTRESOURCEA(GetMenuItemID(child, 2) - 100);
        HRESULT invoked = context->InvokeCommand(&command);
        DestroyMenu(parent); context.Reset(); extension.Reset(); factory.Reset(); FreeLibrary(library);
        return SUCCEEDED(invoked) ? 0 : 39;
    }
    std::vector<std::wstring> expected{L"C:\\Program Files\\資料 & setup.exe", L"C:\\folder\\", L"C:\\quotes\"test", L"C:\\normal.msi"};
    std::wstring command = L"dummy.exe";
    for (const auto& item : expected) command += L" " + quote(item);
    int argc = 0;
    auto argv = CommandLineToArgvW(command.c_str(), &argc);
    assert(argv && argc == static_cast<int>(expected.size() + 1));
    for (size_t i = 0; i < expected.size(); ++i) assert(expected[i] == argv[i + 1]);
    LocalFree(argv);
    assert(DllCanUnloadNow() == S_OK);
    auto menu = new Menu();
    assert(DllCanUnloadNow() == S_FALSE);
    assert(menu->Initialize(nullptr, nullptr, nullptr) == E_INVALIDARG);
    auto data = new DropData(expected);
    assert(menu->Initialize(nullptr, data, nullptr) == S_OK);
    data->Release();
    RegistryFixture registry;
    HMENU parent = CreatePopupMenu();
    auto result = menu->QueryContextMenu(parent, 0, 100, 200, CMF_DEFAULTONLY);
    assert(SUCCEEDED(result) && GetMenuItemCount(parent) == 0);
    result = menu->QueryContextMenu(parent, 0, 100, 200, CMF_NORMAL);
    assert(SUCCEEDED(result) && GetMenuItemCount(parent) == 1);
    auto popup = GetSubMenu(parent, 0);
    int count = GetMenuItemCount(popup);
    assert(count >= 1 && count <= 6);
    UINT next = 100;
    for (int i = 0; i < count; ++i) {
        HMENU child = GetSubMenu(popup, i);
        if (child) {
            assert(GetMenuItemCount(child) == 2);
            wchar_t label[128]{};
            GetMenuStringW(child, 0, label, 128, MF_BYPOSITION); assert(std::wstring(label) == L"Add as Owned");
            GetMenuStringW(child, 1, label, 128, MF_BYPOSITION); assert(std::wstring(label) == L"Add as Reference");
            for (int j = 0; j < 2; ++j) assert(GetMenuItemID(child, j) == next++);
        } else assert(GetMenuItemID(popup, i) == next++);
    }
    assert(static_cast<UINT>(HRESULT_CODE(result)) == next - 100);
    HMENU insufficient = CreatePopupMenu();
    result = menu->QueryContextMenu(insufficient, 0, 100, 99, CMF_NORMAL);
    assert(HRESULT_CODE(result) == 0 && GetMenuItemCount(insufficient) == 0);
    DestroyMenu(insufficient);
    CMINVOKECOMMANDINFO invalid{sizeof(invalid)}; invalid.lpVerb = MAKEINTRESOURCEA(99);
    assert(menu->InvokeCommand(&invalid) == E_INVALIDARG);
    invalid.cbSize = 0; invalid.lpVerb = MAKEINTRESOURCEA(0);
    assert(menu->InvokeCommand(&invalid) == E_INVALIDARG);
    invalid.lpVerb = "untrusted-text";
    assert(menu->InvokeCommand(&invalid) == E_INVALIDARG);
    DestroyMenu(parent);
    wchar_t current[32768]{};
    assert(GetModuleFileNameW(nullptr, current, 32768));
    auto executable = new DropData({std::wstring(current)});
    assert(menu->Initialize(nullptr, executable, nullptr) == S_OK); executable->Release();
    parent = CreatePopupMenu();
    result = menu->QueryContextMenu(parent, 0, 100, 200, CMF_NORMAL);
    assert(HRESULT_CODE(result) == 16);
    auto installerMenu = GetSubMenu(GetSubMenu(parent, 0), 0);
    assert(GetMenuItemCount(installerMenu) == 3);
    wchar_t installTitle[128]{};
    GetMenuStringW(installerMenu, 0, installTitle, 128, MF_BYPOSITION);
    assert(std::wstring(installTitle) == L"Install Application");
    GetMenuStringW(installerMenu, 1, installTitle, 128, MF_BYPOSITION);
    assert(std::wstring(installTitle) == L"Add as Owned");
    GetMenuStringW(installerMenu, 2, installTitle, 128, MF_BYPOSITION);
    assert(std::wstring(installTitle) == L"Add as Reference");
    DestroyMenu(parent);
    registry.restore();
    auto too_many = new DropData(std::vector<std::wstring>(33, L"C:\\a.txt"));
    assert(menu->Initialize(nullptr, too_many, nullptr) == S_OK);
    parent = CreatePopupMenu();
    assert(SUCCEEDED(menu->QueryContextMenu(parent, 0, 100, 200, CMF_NORMAL)));
    assert(GetMenuState(parent, 0, MF_BYPOSITION) & MF_GRAYED);
    DestroyMenu(parent);
    too_many->Release();
    menu->Release();
    assert(DllCanUnloadNow() == S_OK);
    if (arguments == 2 && (std::wstring(options[1]) == L"--registered" || std::wstring(options[1]) == L"--inspect-registered" || std::wstring(options[1]) == L"--inspect-registered-installer")) {
        const bool inspect = std::wstring(options[1]) != L"--registered";
        const bool installer = std::wstring(options[1]) == L"--inspect-registered-installer";
        HRESULT initialized = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
        if (FAILED(initialized)) return 10;
        ComPtr<IShellExtInit> registered;
        result = CoCreateInstance(ClassId, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&registered));
        if (FAILED(result)) { std::wcerr << L"Registered COM activation failed: " << std::hex << result; CoUninitialize(); return 11; }
        wchar_t currentExecutable[32768]{};
        if (!GetModuleFileNameW(nullptr, currentExecutable, 32768)) return 18;
        auto selection = installer ? new DropData({std::wstring(currentExecutable)}) : new DropData({L"C:\\資料 & example.txt", L"D:\\other\\setup.MSI"});
        result = registered->Initialize(nullptr, selection, nullptr); selection->Release();
        if (FAILED(result)) return 12;
        ComPtr<IContextMenu> context; result = registered.As(&context);
        if (FAILED(result)) return 13;
        HMENU actual = CreatePopupMenu();
        result = context->QueryContextMenu(actual, 0, 100, 200, CMF_NORMAL);
        if (FAILED(result) || GetMenuItemCount(actual) != 1) return 14;
        wchar_t title[256]{}; GetMenuStringW(actual, 0, title, 256, MF_BYPOSITION);
        if (std::wstring(title) != L"Casklet") return 15;
        HMENU submenu = GetSubMenu(actual, 0);
        int entries = GetMenuItemCount(submenu);
        if ((!inspect && entries != 6) || entries < 1 || entries > 6) return 16;
        int nestedCasks = 0;
        for (int i = 0; i < entries; ++i) {
            if (!inspect) {
                GetMenuStringW(submenu, i, title, 256, MF_BYPOSITION);
                std::wcout << title << L"\n";
            }
            HMENU child = GetSubMenu(submenu, i);
            if (!child) {
                GetMenuStringW(submenu, i, title, 256, MF_BYPOSITION);
                if (i != entries - 1 || (std::wstring(title) != L"More casks…" && std::wstring(title) != L"Choose or create a cask…" && std::wstring(title) != L"Choose a cask in Casklet…")) return 19;
                continue;
            }
            ++nestedCasks;
            if (GetMenuItemCount(child) != (installer ? 3 : 2)) return 20;
            const wchar_t* labels[] = {L"Install Application", L"Add as Owned", L"Add as Reference"};
            for (int j = 0; j < GetMenuItemCount(child); ++j) {
                GetMenuStringW(child, j, title, 256, MF_BYPOSITION);
                if (std::wstring(title) != labels[j + (installer ? 0 : 1)]) return 21;
            }
        }
        if (inspect) std::wcout << L"Registered Cask submenus verified: " << nestedCasks << L"; actions per Cask: " << (installer ? 3 : 2) << L"\n";
        MENUITEMINFOW icon{sizeof(icon)}; icon.fMask = MIIM_BITMAP;
        if (!GetMenuItemInfoW(actual, 0, TRUE, &icon) || !icon.hbmpItem) return 17;
        DestroyMenu(actual); context.Reset(); registered.Reset(); CoUninitialize();
        std::wcout << L"Registered native shell, icon and mixed-location selection: PASS\n";
    }
    return 0;
}
