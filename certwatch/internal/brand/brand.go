package brand

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"image"
	"image/color"
	"image/png"
	"math"
	"strconv"
	"strings"
	"sync"
)

var (
	Background = color.NRGBA{R: 0x1F, G: 0x3A, B: 0x5F, A: 0xFF}
	Foreground = color.NRGBA{R: 0xF5, G: 0xF7, B: 0xFA, A: 0xFF}
	Accent     = color.NRGBA{R: 0x4F, G: 0xC3, B: 0xA1, A: 0xFF}
)

type shape struct {
	x, y, w, h, r float64
	tint          color.NRGBA
}

var motif = []shape{
	{0.22, 0.26, 0.10, 0.10, 0.05, Accent},
	{0.40, 0.26, 0.38, 0.10, 0.05, Foreground},
	{0.22, 0.45, 0.10, 0.10, 0.05, Accent},
	{0.40, 0.45, 0.38, 0.10, 0.05, Foreground},
	{0.22, 0.64, 0.10, 0.10, 0.05, Accent},
	{0.40, 0.64, 0.24, 0.10, 0.05, Foreground},
}

var smallMotif = []shape{
	{0.19, 0.25, 0.16, 0.19, 0.06, Accent},
	{0.44, 0.25, 0.37, 0.19, 0.06, Foreground},
	{0.19, 0.56, 0.16, 0.19, 0.06, Accent},
	{0.44, 0.56, 0.37, 0.19, 0.06, Foreground},
}

var plate = shape{0.04, 0.04, 0.92, 0.92, 0.20, Background}

const smallLimit = 24

var Sizes = []int{16, 24, 32, 48, 64, 128, 256, 512, 1024}

func parts(size int) []shape {
	if size <= smallLimit {
		return smallMotif
	}
	return motif
}

func coverage(px, py float64, s shape) float64 {
	const samples = 4
	inside := 0
	for sy := 0; sy < samples; sy++ {
		for sx := 0; sx < samples; sx++ {
			x := px + (float64(sx)+0.5)/samples
			y := py + (float64(sy)+0.5)/samples
			dx := math.Max(math.Max(s.x+s.r-x, x-(s.x+s.w-s.r)), 0)
			dy := math.Max(math.Max(s.y+s.r-y, y-(s.y+s.h-s.r)), 0)
			if x >= s.x && x <= s.x+s.w && y >= s.y && y <= s.y+s.h && dx*dx+dy*dy <= s.r*s.r {
				inside++
			}
		}
	}
	return float64(inside) / (samples * samples)
}

func blend(base color.NRGBA, top color.NRGBA, share float64) color.NRGBA {
	mix := func(a, b uint8) uint8 { return uint8(math.Round(float64(a)*(1-share) + float64(b)*share)) }
	return color.NRGBA{R: mix(base.R, top.R), G: mix(base.G, top.G), B: mix(base.B, top.B), A: 0xFF}
}

func Icon(size int) *image.NRGBA {
	picture := image.NewNRGBA(image.Rect(0, 0, size, size))
	scale := float64(size)
	scaled := func(s shape) shape {
		return shape{s.x * scale, s.y * scale, s.w * scale, s.h * scale, s.r * scale, s.tint}
	}
	plate := scaled(plate)
	motif := parts(size)
	for y := 0; y < size; y++ {
		for x := 0; x < size; x++ {
			share := coverage(float64(x), float64(y), plate)
			if share == 0 {
				continue
			}
			pixel := Background
			for _, part := range motif {
				if inside := coverage(float64(x), float64(y), scaled(part)); inside > 0 {
					pixel = blend(pixel, part.tint, inside)
				}
			}
			pixel.A = uint8(math.Round(share * 255))
			picture.SetNRGBA(x, y, pixel)
		}
	}
	return picture
}

var encoded sync.Map

func PNG(size int) []byte {
	if cached, found := encoded.Load(size); found {
		return cached.([]byte)
	}
	var buffer bytes.Buffer
	if err := png.Encode(&buffer, Icon(size)); err != nil {
		return nil
	}
	encoded.Store(size, buffer.Bytes())
	return buffer.Bytes()
}

func hex(tint color.NRGBA) string {
	return fmt.Sprintf("#%02X%02X%02X", tint.R, tint.G, tint.B)
}

func svg(title string, shapes []shape) string {
	var builder strings.Builder
	const unit = 256.0
	number := func(value float64) string { return strconv.FormatFloat(math.Round(value*unit*100)/100, 'f', -1, 64) }
	builder.WriteString("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 256 256\" width=\"256\" height=\"256\" role=\"img\">\n")
	builder.WriteString("  <title>" + title + "</title>\n")
	for _, part := range append([]shape{plate}, shapes...) {
		builder.WriteString("  <rect x=\"" + number(part.x) + "\" y=\"" + number(part.y) + "\" width=\"" + number(part.w) + "\" height=\"" + number(part.h) + "\" rx=\"" + number(part.r) + "\" fill=\"" + hex(part.tint) + "\"/>\n")
	}
	builder.WriteString("</svg>\n")
	return builder.String()
}

func SVG(title string) string { return svg(title, motif) }

func SmallSVG(title string) string { return svg(title, smallMotif) }

func ICO() []byte {
	sizes := []int{16, 32, 48, 256}
	var buffer bytes.Buffer
	write := func(value any) { _ = binary.Write(&buffer, binary.LittleEndian, value) }
	write([]uint16{0, 1, uint16(len(sizes))})
	offset := 6 + 16*len(sizes)
	pictures := make([][]byte, len(sizes))
	for index, size := range sizes {
		pictures[index] = PNG(size)
		write([]uint8{uint8(size % 256), uint8(size % 256), 0, 0})
		write([]uint16{1, 32})
		write([]uint32{uint32(len(pictures[index])), uint32(offset)})
		offset += len(pictures[index])
	}
	for _, picture := range pictures {
		buffer.Write(picture)
	}
	return buffer.Bytes()
}

func ICNS() []byte {
	entries := []struct {
		kind string
		size int
	}{{"icp4", 16}, {"icp5", 32}, {"icp6", 64}, {"ic07", 128}, {"ic08", 256}, {"ic09", 512}, {"ic10", 1024}}
	var body bytes.Buffer
	for _, entry := range entries {
		picture := PNG(entry.size)
		body.WriteString(entry.kind)
		_ = binary.Write(&body, binary.BigEndian, uint32(len(picture)+8))
		body.Write(picture)
	}
	var buffer bytes.Buffer
	buffer.WriteString("icns")
	_ = binary.Write(&buffer, binary.BigEndian, uint32(body.Len()+8))
	buffer.Write(body.Bytes())
	return buffer.Bytes()
}

func DesktopEntry(name, comment, slug string) string {
	return "[Desktop Entry]\nType=Application\nVersion=1.0\nName=" + name + "\nComment=" + comment + "\nExec=" + slug + "\nIcon=" + slug + "\nTerminal=false\nCategories=Utility;\n"
}

func Files(name, comment, slug string) map[string][]byte {
	files := map[string][]byte{
		"icon.svg":               []byte(SVG(name + " icon")),
		"icon-16.svg":            []byte(SmallSVG(name + " icon, simplified for small sizes")),
		"icon.ico":               ICO(),
		"icon.icns":              ICNS(),
		"icon.png":               PNG(512),
		slug + ".desktop":        []byte(DesktopEntry(name, comment, slug)),
		"linux/" + slug + ".svg": []byte(SVG(name + " icon")),
		"linux/" + slug + ".png": PNG(256),
	}
	for _, size := range Sizes {
		files["png/icon-"+strconv.Itoa(size)+".png"] = PNG(size)
	}
	return files
}
