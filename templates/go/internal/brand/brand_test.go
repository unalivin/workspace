package brand

import (
	"bytes"
	"encoding/binary"
	"encoding/xml"
	"image"
	"image/png"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/NexzaDev/templateapp/internal/meta"
)

func TestIconHasThePlateAndTheMotifAtEverySize(t *testing.T) {
	for _, size := range Sizes {
		decoded, err := png.Decode(bytes.NewReader(PNG(size)))
		if err != nil {
			t.Fatal(err)
		}
		picture, isNRGBA := decoded.(*image.NRGBA)
		if !isNRGBA {
			t.Fatalf("size %d decodes as %T", size, decoded)
		}
		if picture.Bounds().Dx() != size || picture.Bounds().Dy() != size {
			t.Fatalf("size %d has bounds %v", size, picture.Bounds())
		}
		if picture.NRGBAAt(0, 0).A != 0 {
			t.Errorf("size %d: the corner is not transparent", size)
		}
		if center := picture.NRGBAAt(size/2, size/8); center != Background {
			t.Errorf("size %d: the plate is %v", size, center)
		}
		tints := map[[3]uint8]bool{}
		for y := 0; y < size; y++ {
			for x := 0; x < size; x++ {
				pixel := picture.NRGBAAt(x, y)
				tints[[3]uint8{pixel.R, pixel.G, pixel.B}] = true
			}
		}
		for _, wanted := range [][3]uint8{{Foreground.R, Foreground.G, Foreground.B}, {Accent.R, Accent.G, Accent.B}} {
			if !tints[wanted] {
				t.Errorf("size %d lacks the tint %v", size, wanted)
			}
		}
	}
}

func TestSmallSizesUseTheSimplifiedMotif(t *testing.T) {
	if len(parts(16)) >= len(parts(32)) {
		t.Fatal("the 16 pixel icon is not simplified")
	}
	if strings.Count(SmallSVG("x"), "<rect") != len(smallMotif)+1 || strings.Count(SVG("x"), "<rect") != len(motif)+1 {
		t.Fatal("the SVG sources do not match the drawn shapes")
	}
}

func TestSVGIsWellFormed(t *testing.T) {
	for _, source := range []string{SVG("Icon"), SmallSVG("Icon")} {
		decoder := xml.NewDecoder(strings.NewReader(source))
		for {
			if _, err := decoder.Token(); err != nil {
				if err.Error() != "EOF" {
					t.Fatal(err)
				}
				break
			}
		}
		if !strings.Contains(source, "<title>Icon</title>") || strings.Contains(source, "<image") {
			t.Fatal("the SVG lacks a title or embeds a bitmap")
		}
	}
}

func TestICOListsValidPictures(t *testing.T) {
	data := ICO()
	var header [3]uint16
	if err := binary.Read(bytes.NewReader(data), binary.LittleEndian, &header); err != nil {
		t.Fatal(err)
	}
	if header[0] != 0 || header[1] != 1 || header[2] != 4 {
		t.Fatalf("header %v", header)
	}
	for index := 0; index < int(header[2]); index++ {
		entry := data[6+16*index:]
		length := binary.LittleEndian.Uint32(entry[8:12])
		offset := binary.LittleEndian.Uint32(entry[12:16])
		picture, err := png.Decode(bytes.NewReader(data[offset : offset+length]))
		if err != nil {
			t.Fatal(err)
		}
		width := int(entry[0])
		if width == 0 {
			width = 256
		}
		if picture.Bounds().Dx() != width {
			t.Fatalf("entry %d says %d but holds %d", index, width, picture.Bounds().Dx())
		}
	}
}

func TestICNSListsValidPictures(t *testing.T) {
	data := ICNS()
	if string(data[:4]) != "icns" || int(binary.BigEndian.Uint32(data[4:8])) != len(data) {
		t.Fatal("the icns header is wrong")
	}
	found := 0
	for position := 8; position < len(data); {
		length := int(binary.BigEndian.Uint32(data[position+4 : position+8]))
		if _, err := png.Decode(bytes.NewReader(data[position+8 : position+length])); err != nil {
			t.Fatalf("%s: %v", data[position:position+4], err)
		}
		position += length
		found++
	}
	if found != 7 {
		t.Fatalf("%d pictures", found)
	}
}

func TestCommittedAssetsMatchTheGenerator(t *testing.T) {
	files := Files(meta.AppName, meta.Tagline, meta.Slug)
	if len(files) != 8+len(Sizes) {
		t.Fatalf("%d files", len(files))
	}
	for name, wanted := range files {
		current, err := os.ReadFile(filepath.Join("..", "..", "assets", filepath.FromSlash(name)))
		if err != nil {
			t.Fatalf("assets/%s is missing: run make assets", name)
		}
		if !bytes.Equal(current, wanted) {
			t.Errorf("assets/%s is out of date: run make assets", name)
		}
	}
	entry := string(files[meta.Slug+".desktop"])
	for _, line := range []string{"Name=" + meta.AppName, "Exec=" + meta.Slug, "Icon=" + meta.Slug, "Type=Application"} {
		if !strings.Contains(entry, line+"\n") {
			t.Errorf("the desktop entry lacks %q", line)
		}
	}
}
