"""生成简洁的小火箭应用图标 (1024×1024 PNG)。"""
from PIL import Image, ImageDraw, ImageFilter

S = 1024
BG = (108, 92, 231)  # #6C5CE7


def rocket_alpha(size):
    """绘制与托盘图标一致的简洁火箭，返回 alpha Image。"""
    scale = size / 22.0
    img = Image.new("L", (size, size), 0)

    def right_edge(px, py):
        row = int(py)
        body = {0: 10.5, 1: 9.5, 2: 8.5, 3: 7.5,
                4: 5.5, 5: 5.5, 6: 5.5, 7: 5.5,
                8: 5.5, 9: 5.5, 10: 5.5, 11: 5.5,
                12: 5.5, 13: 7.5, 14: 6.5, 15: 5.5,
                16: 4.5, 17: 3.5, 18: 3.5, 19: 2.5}.get(row, 0.0)
        fin = {16: 10.5, 17: 10.5, 18: 9.5}.get(row, body)
        return max(fin, body) - px

    def left_edge(px, py):
        row = int(py)
        body = {0: 10.5, 1: 11.5, 2: 12.5, 3: 13.5,
                4: 15.5, 5: 15.5, 6: 15.5, 7: 15.5,
                8: 15.5, 9: 15.5, 10: 15.5, 11: 15.5,
                12: 15.5, 13: 13.5, 14: 14.5, 15: 15.5,
                16: 16.5, 17: 17.5, 18: 17.5, 19: 18.5}.get(row, 0.0)
        fin = {16: 10.5, 17: 10.5, 18: 11.5}.get(row, body)
        return px - min(fin, body)

    for y in range(size):
        for x in range(size):
            px, py = x + 0.5, y + 0.5
            d = min(left_edge(px / scale, py / scale),
                    right_edge(px / scale, py / scale))
            a = max(0.0, min(1.0, 1.0 - max(0.0, d)))
            img.putpixel((x, y), int(a * 255))

    return img.filter(ImageFilter.GaussianBlur(6))


def main():
    bg = Image.new("RGBA", (S, S), (*BG, 255))

    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, S - 1, S - 1], radius=160, fill=255)

    rocket = rocket_alpha(S)
    rocket_layer = Image.new("RGBA", (S, S), (255, 255, 255, 0))
    rocket_layer.putalpha(rocket)

    out = Image.alpha_composite(bg, rocket_layer)
    out.putalpha(mask)
    out.save("assets/app_icon.png")
    print("已生成 assets/app_icon.png")


if __name__ == "__main__":
    main()
