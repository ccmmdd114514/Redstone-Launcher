"""创建英文路径的目录链接，绕开 MinGW 工具链对中文路径的处理问题。

junction 是 NTFS 的目录链接，创建不需要管理员权限，且对程序完全透明：
从链接路径进去，cwd、target、各种中间产物路径全是英文。
源码实际仍然在 D:\\数据\\Programming\\Redstone-Launcher。
"""

import os
import subprocess

# 链接建在用户目录下、目标指向本仓库所在目录：两者都由运行时推导，
# 不写死任何用户名或磁盘位置，换机器 clone 后直接可用。
LINK = os.path.join(os.environ.get("USERPROFILE", ""), "redstone")
TARGET = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def main() -> None:
    if os.path.isdir(LINK):
        print(f"[已存在] {LINK}")
        try:
            os.rmdir(LINK)  # junction 可以直接 rmdir 删掉，不动目标目录
            print("  已移除旧链接（目标目录未受影响）")
        except OSError as exc:
            raise SystemExit(f"[中止] 删不掉旧链接：{exc}") from exc

    if not os.path.isdir(TARGET):
        raise SystemExit(f"[中止] 目标目录不存在：{TARGET}")

    result = subprocess.run(
        ["cmd", "/c", "mklink", "/J", LINK, TARGET],
        capture_output=True,
        text=True,
        encoding="gbk",
        errors="replace",
    )
    print(result.stdout.strip() or result.stderr.strip())

    if os.path.isdir(LINK):
        print(f"\n[完成] {LINK}  ->  {TARGET}")
        print("\n链接里的文件：")
        for name in sorted(os.listdir(LINK))[:10]:
            print(f"  {name}")
    else:
        raise SystemExit("[失败] 链接没建成")


if __name__ == "__main__":
    main()
