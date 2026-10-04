//! 终端呈现层：彩色输出、单键读取、字符画标题。
//!
//! Windows 控制台默认不解释 ANSI 转义序列，需先开启
//! `ENABLE_VIRTUAL_TERMINAL_PROCESSING`；读取单个按键则需临时关闭行缓冲与回显。
//! 两者都通过 windows-sys 直接调用控制台 API，不引入额外的终端库。

use std::sync::atomic::{AtomicBool, Ordering};
use windows_sys::Win32::System::Console::{
    GetConsoleMode, GetStdHandle, ReadConsoleW, SetConsoleMode, CONSOLE_MODE, ENABLE_ECHO_INPUT,
    ENABLE_LINE_INPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};

/// 终端是否接受 ANSI 转义序列。初始化失败时，所有着色自动退化为纯文本。
static COLOR_ON: AtomicBool = AtomicBool::new(false);

/// 启动时调用一次：尝试启用 ANSI 彩色输出。
pub fn init() {
    let ok = unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode: CONSOLE_MODE = 0;
        if GetConsoleMode(handle, &mut mode) == 0 {
            false
        } else {
            SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) != 0
        }
    };
    COLOR_ON.store(ok, Ordering::Relaxed);
}

fn paint(code: &str, text: &str) -> String {
    if COLOR_ON.load(Ordering::Relaxed) {
        format!("\u{1b}[{code}m{text}\u{1b}[0m")
    } else {
        text.to_string()
    }
}

pub fn red(text: &str) -> String {
    paint("31", text)
}

pub fn bright_red(text: &str) -> String {
    paint("91", text)
}

pub fn green(text: &str) -> String {
    paint("32", text)
}

pub fn yellow(text: &str) -> String {
    paint("33", text)
}

pub fn cyan(text: &str) -> String {
    paint("36", text)
}

pub fn gray(text: &str) -> String {
    paint("90", text)
}

pub fn bold(text: &str) -> String {
    paint("1", text)
}

pub fn bold_red(text: &str) -> String {
    paint("1;31", text)
}

/// 读取单个按键：不回显、不需要回车。
///
/// 返回 `None` 表示读到 EOF（Ctrl+Z）或当前不是交互式终端（例如被重定向）。
pub fn read_key() -> Option<char> {
    unsafe {
        let handle = GetStdHandle(STD_INPUT_HANDLE);
        let mut mode: CONSOLE_MODE = 0;
        if GetConsoleMode(handle, &mut mode) == 0 {
            return None;
        }
        // 关掉行缓冲与回显，按键即可立即返回
        if SetConsoleMode(handle, mode & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT)) == 0 {
            return None;
        }
        let first = read_one(handle);
        // 方向键、功能键是「前缀 0x00 / 0xE0 + 扫描码」两个字符。只读第一个的话，
        // 扫描码会留在缓冲区里，被下一次 read_key 当成用户按键吞掉，出现「按了没反应、
        // 再按又乱跳」。这里顺手把第二个字符也读掉，返回它（不是数字，会被判为无效键）。
        let ch = match first {
            Some(0) | Some(0xE0) => {
                let _ = read_one(handle);
                Some(0)
            }
            other => other,
        };
        // 读完立刻恢复原模式
        SetConsoleMode(handle, mode);
        ch.and_then(char::from_u32)
    }
}

/// 从控制台读 1 个 UTF-16 码元的原始值。调用方负责开关控制台模式。
unsafe fn read_one(handle: *mut core::ffi::c_void) -> Option<u32> {
    let mut buf = [0u16; 1];
    let mut read: u32 = 0;
    let ok = ReadConsoleW(
        handle,
        buf.as_mut_ptr() as *mut core::ffi::c_void,
        1,
        &mut read,
        std::ptr::null(),
    );
    if ok == 0 || read == 0 {
        None
    } else {
        Some(buf[0] as u32)
    }
}

/// 字符画标题：一个红石方块图案配名称与版本号。
///
/// 全部使用方块字符（U+2588 / U+2593），不依赖外部图片资源，
/// 任何等宽字体都能正常显示。
pub fn banner() -> String {
    let logo = [
        r"      ██████████████",
        r"    ██▓▓▓▓▓▓▓▓▓▓▓▓██",
        r"    ██▓▓██▓▓▓▓██▓▓██",
        r"    ██▓▓▓▓██▓▓▓▓▓▓██",
        r"    ██▓▓██▓▓▓▓██▓▓██",
        r"    ██▓▓▓▓▓▓▓▓▓▓▓▓██",
        r"      ██████████████",
    ];
    // 名称用亮红加重，其余保持常规前景色，避免整屏都是彩色刺眼。
    let text = [
        String::new(),
        bright_red("  红石启动器"),
        String::from("  Redstone Launcher"),
        gray(&format!("  版本 {}", env!("CARGO_PKG_VERSION"))),
        String::new(),
        String::from("  Minecraft Java 版启动器"),
        String::new(),
    ];
    let mut out = String::new();
    for (icon, label) in logo.iter().zip(text.iter()) {
        out.push_str(&bright_red(icon));
        out.push_str(label);
        out.push('\n');
    }
    out
}
