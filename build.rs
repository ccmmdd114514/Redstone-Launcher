// 构建脚本：把 assets\icon.ico 嵌入 redstone.exe 的 Windows 资源段，
// 并填入文件版本信息。
//
// 效果：资源管理器、任务栏、开始菜单里显示的是红石方块图标，
// 而不是 Windows 默认的那个空白可执行文件图标；文件属性里也能看到版本号。
//
// 图标由 tools\make-icons.py 生成，内含 16/24/32/48/64/128/256 七档尺寸，
// 小尺寸与高分屏都能正确缩放而不糊。
//
// 若目标平台不是 Windows（例如在 Linux 上做 CI 检查），整段跳过。

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");

    #[cfg(target_os = "windows")]
    {
        let icon = std::path::Path::new("assets/icon.ico");
        assert!(
            icon.is_file(),
            "找不到图标文件 {}，请先运行 tools\\make-icons.py 生成",
            icon.display()
        );

        let version = env!("CARGO_PKG_VERSION");
        let mut res = winresource::WindowsResource::new();
        res.set_icon(icon.to_str().expect("图标路径不是合法 UTF-8"));
        res.set("FileDescription", "Redstone Launcher - Minecraft Java Edition launcher");
        res.set("ProductName", "Redstone Launcher");
        res.set("FileVersion", version);
        res.set("ProductVersion", version);
        res.set("LegalCopyright", "Copyright (c) 2026 Redstone Launcher contributors");

        if let Err(err) = res.compile() {
            // 图标编不进去不该让整个构建失败——退回到无图标构建并留下警告，
            // 免得图形资源的问题把整个编译流程卡死。
            println!("cargo:warning=图标资源编译失败，本次构建的 exe 将没有图标：{err}");
        }
    }
}
