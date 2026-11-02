from pathlib import Path
from PIL import Image, ImageDraw

root = Path(__file__).parent
labels = ['0', '7', '37', '100', '999', 'unknown', '!']
sizes = [16, 20, 24, 32, 48, 64]
sheet = Image.new('RGB', (1120, 680), '#858585')
draw = ImageDraw.Draw(sheet)
for panel, background in enumerate(['#181818', '#fafafa']):
    left = 10 + panel * 560
    draw.rectangle((left, 10, left + 540, 670), fill=background)
    color = 'white' if panel == 0 else 'black'
    draw.text((left + 10, 16), 'Actual pixel sizes: 16, 20, 24, 32, 48, 64', fill=color)
    for row, label in enumerate(labels):
        draw.text((left + 8, 62 + row * 85), label, fill=color)
        icon = Image.open(root / (label + '.ico'))
        assert icon.ico.sizes() == {(size, size) for size in sizes}
        for column, size in enumerate(sizes):
            image = icon.ico.getimage((size, size))
            sheet.paste(image, (left + 80 + column * 75 + (64-size)//2, 45 + row * 85 + (64-size)//2), image)
sheet.save(root / 'native-sizes.png')
small = Image.new('RGB', (7 * 84, 180), '#858585')
draw = ImageDraw.Draw(small)
for row, background in enumerate(['#181818', '#fafafa']):
    for col, label in enumerate(labels):
        image = Image.open(root / (label + '.ico')).ico.getimage((16, 16))
        cell = Image.new('RGBA', (16, 16), background)
        cell.alpha_composite(image)
        small.paste(cell.convert('RGB').resize((64, 64), Image.Resampling.NEAREST), (col*84+10, row*90+5))
        draw.text((col*84+10, row*90+72), label, fill='black')
small.save(root / '16px-magnified.png')
