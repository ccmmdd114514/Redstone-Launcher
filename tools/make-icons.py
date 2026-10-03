"""从一张 PNG 生成两样东西：exe 用的多尺寸 .ico，仓库头像用的大图 png。

用法（需带 Pillow 的 Python）：

    "C:\\Program Files\\Python314\\python.exe" tools\\make-icons.py "C:\\path\\to\\icon.png"

输出：
    assets\\icon.ico   16/24/32/48/64/128/256 七档，嵌进 exe 资源用
    assets\\icon.png   512×512，GitHub 仓库头像与文档配图用
"""

import pathlib
import sys

try:
    from PIL import Image
except ImportError:
    raise SystemExit(
        "[中止] 没装 Pillow。托管 Python 不带它，请改用系统解释器：\n"
        "  \"C:\\Program Files\\Python314\\python.exe\" tools\\make-icons.py <源PNG>"
    )


def square(img: Image.Image) -> Image.Image:
    """补透明边把长图压成正方形，避免拉伸变形。"""
    w, h = img.size
    if w == h:
        return img
    side = max(w, h)
    canvas = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    canvas.paste(img, ((side - w) // 2, (side - h) // 2))
    return canvas


def main() -> None:
    if len(sys.argv) < 2:
        raise SystemExit("[中止] 请传入源 PNG 路径")

    src = pathlib.Path(sys.argv[1])
    if not src.is_file():
        raise SystemExit(f"[中止] 找不到源图：{src}")

    out_dir = pathlib.Path(__file__).resolve().parent.parent / "assets"
    out_dir.mkdir(exist_ok=True)

    img = square(Image.open(src).convert("RGBA"))
    print(f"[读入] {src.name}  {img.size[0]}×{img.size[1]}")

    big = img.resize((512, 512), Image.LANCZOS)
    png_out = out_dir / "icon.png"
    big.save(png_out, optimize=True)
    print(f"[完成] {png_out.relative_to(out_dir.parent)}  512×512  {png_out.stat().st_size} 字节")

    ico_out = out_dir / "icon.ico"
    img.save(ico_out, sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    print(f"[完成] {ico_out.relative_to(out_dir.parent)}  七档尺寸  {ico_out.stat().st_size} 字节")


if __name__ == "__main__":
    main()
