"""Generate the project tray rocket icon."""

from PIL import Image, ImageDraw, ImageFilter


CANVAS = 128
SCALE = 1
S = 512


def vertical_gradient(size, top, bottom):
    image = Image.new("RGBA", size, top)
    pixels = image.load()
    height = size[1]
    for y in range(height):
        t = y / max(1, height - 1)
        color = tuple(int(top[i] * (1 - t) + bottom[i] * t) for i in range(4))
        for x in range(size[0]):
            pixels[x, y] = color
    return image


def draw_rocket():
    layer = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)

    body_mask = Image.new("L", (S, S), 0)
    mask_draw = ImageDraw.Draw(body_mask)
    mask_draw.polygon([(256 * SCALE, 28 * SCALE), (164 * SCALE, 142 * SCALE), (348 * SCALE, 142 * SCALE)], fill=255)
    mask_draw.rounded_rectangle(
        [164 * SCALE, 116 * SCALE, 348 * SCALE, 358 * SCALE],
        radius=58 * SCALE,
        fill=255,
    )

    body_fill = vertical_gradient(
        (S, S),
        (255, 255, 255, 255),
        (74, 111, 255, 255),
    )
    layer.alpha_composite(Image.composite(body_fill, Image.new("RGBA", (S, S), (0, 0, 0, 0)), body_mask))

    draw.line(
        [(256 * SCALE, 28 * SCALE), (164 * SCALE, 142 * SCALE), (164 * SCALE, 292 * SCALE)],
        fill=(46, 60, 122, 190),
        width=5 * SCALE,
        joint="curve",
    )
    draw.line(
        [(256 * SCALE, 28 * SCALE), (348 * SCALE, 142 * SCALE), (348 * SCALE, 292 * SCALE)],
        fill=(46, 60, 122, 170),
        width=5 * SCALE,
        joint="curve",
    )

    draw.polygon(
        [(175 * SCALE, 272 * SCALE), (82 * SCALE, 386 * SCALE), (190 * SCALE, 350 * SCALE)],
        fill=(255, 111, 97, 255),
    )
    draw.polygon(
        [(337 * SCALE, 272 * SCALE), (430 * SCALE, 386 * SCALE), (322 * SCALE, 350 * SCALE)],
        fill=(255, 111, 97, 255),
    )

    draw.polygon(
        [(225 * SCALE, 350 * SCALE), (256 * SCALE, 484 * SCALE), (287 * SCALE, 350 * SCALE)],
        fill=(255, 122, 45, 255),
    )
    draw.polygon(
        [(239 * SCALE, 355 * SCALE), (256 * SCALE, 444 * SCALE), (273 * SCALE, 355 * SCALE)],
        fill=(255, 221, 88, 255),
    )

    draw.ellipse(
        [210 * SCALE, 154 * SCALE, 302 * SCALE, 246 * SCALE],
        fill=(35, 55, 130, 255),
    )
    draw.ellipse(
        [224 * SCALE, 168 * SCALE, 288 * SCALE, 232 * SCALE],
        fill=(114, 222, 255, 255),
    )
    draw.ellipse(
        [237 * SCALE, 178 * SCALE, 260 * SCALE, 201 * SCALE],
        fill=(255, 255, 255, 200),
    )

    rotated = layer.rotate(-38, resample=Image.Resampling.BICUBIC, expand=True)
    shadow = Image.new("RGBA", rotated.size, (0, 0, 0, 0))
    shadow_alpha = rotated.getchannel("A").filter(ImageFilter.GaussianBlur(7 * SCALE))
    shadow.putalpha(shadow_alpha.point(lambda alpha: int(alpha * 0.22)))

    out = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    x = (S - rotated.width) // 2
    y = (S - rotated.height) // 2 - 8 * SCALE
    out.alpha_composite(shadow, (x + 8 * SCALE, y + 10 * SCALE))
    out.alpha_composite(rotated, (x, y))
    return out.resize((CANVAS, CANVAS), Image.Resampling.LANCZOS)


def main():
    icon = draw_rocket()
    icon.save("assets/tray_rocket.png")
    print("Generated assets/tray_rocket.png")


if __name__ == "__main__":
    main()
