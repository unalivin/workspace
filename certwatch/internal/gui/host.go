package gui

import (
	"context"
	"errors"
	"sync"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/layout"
	fynetheme "fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

	"github.com/unalivin/certwatch/internal/brand"
	"github.com/unalivin/certwatch/internal/launcher"
	"github.com/unalivin/certwatch/internal/meta"
	"github.com/unalivin/certwatch/internal/settings"
	"github.com/unalivin/certwatch/internal/theme"
)

type Host struct {
	create  func() fyne.App
	lock    sync.Mutex
	started chan func()
	stopped chan struct{}
	once    sync.Once
	fyneApp fyne.App
	window  fyne.Window
	running bool
}

func NewHost(create func() fyne.App) *Host {
	return &Host{create: create, started: make(chan func(), 1), stopped: make(chan struct{})}
}

func (h *Host) Loop() {
	select {
	case <-h.stopped:
		return
	case first := <-h.started:
		h.fyneApp = h.create()
		h.fyneApp.SetIcon(appIcon())
		applyDefaultSkin(h.fyneApp)
		h.window = h.fyneApp.NewWindow(meta.AppName)
		h.window.SetMaster()
		first()
		h.fyneApp.Run()
	}
}

func (h *Host) Stop() {
	h.once.Do(func() {
		h.lock.Lock()
		running := h.running
		h.lock.Unlock()
		close(h.stopped)
		if running {
			fyne.Do(func() { h.fyneApp.Quit() })
		}
	})
}

func (h *Host) submit(work func()) error {
	h.lock.Lock()
	first := !h.running
	h.running = true
	h.lock.Unlock()
	select {
	case <-h.stopped:
		return errors.New("the application is closing")
	default:
	}
	if first {
		h.started <- work
		return nil
	}
	fyne.Do(work)
	return nil
}

func (h *Host) Chooser(defaultMode string) (launcher.PromptResult, error) {
	result := make(chan launcher.PromptResult, 1)
	err := h.submit(func() {
		showChooser(h.window, defaultMode, func(chosen launcher.PromptResult) {
			h.window.Hide()
			result <- chosen
		})
	})
	if err != nil {
		return launcher.PromptResult{}, err
	}
	return <-result, nil
}

func (h *Host) Dashboard(ctx context.Context, options Options) (string, error) {
	result := make(chan string, 1)
	err := h.submit(func() {
		board := newDashboard(ctx, h.fyneApp, h.window, options, func(switchTo string) {
			h.window.Hide()
			result <- switchTo
		})
		h.window.SetTitle(meta.AppName)
		h.window.SetFixedSize(false)
		h.window.SetCloseIntercept(func() { board.close("") })
		width, height := board.values[settings.KeyWindowWidth].Int(), board.values[settings.KeyWindowHeight].Int()
		h.window.Resize(fyne.NewSize(float32(width), float32(height)))
		h.window.CenterOnScreen()
		h.window.Show()
		board.watch()
	})
	if err != nil {
		return "", err
	}
	return <-result, nil
}

func appIcon() fyne.Resource {
	return fyne.NewStaticResource("icon.png", brand.PNG(256))
}

func appIconImage(size float32) *canvas.Image {
	image := canvas.NewImageFromResource(appIcon())
	image.FillMode = canvas.ImageFillContain
	image.SetMinSize(fyne.NewSize(size, size))
	return image
}

func applyDefaultSkin(fyneApp fyne.App) {
	mode := theme.ModeLight
	if fyneApp.Settings().ThemeVariant() == fynetheme.VariantDark {
		mode = theme.ModeDark
	}
	fyneApp.Settings().SetTheme(skin{tokens: theme.Resolve(theme.Options{Name: "slate", Mode: mode, TextScale: 100})})
}

type chooser struct {
	icon      *canvas.Image
	remember  *widget.Check
	dashboard *widget.Button
	terminal  *widget.Button
	quit      *widget.Button
	content   fyne.CanvasObject
}

func newChooser(defaultMode string, done func(launcher.PromptResult)) *chooser {
	c := &chooser{remember: widget.NewCheck("Remember my choice", nil)}
	choose := func(mode string) func() {
		return func() { done(launcher.PromptResult{Mode: mode, Remember: c.remember.Checked}) }
	}
	c.dashboard = widget.NewButton("Dashboard (graphical window)", choose(launcher.ModeGUI))
	c.terminal = widget.NewButton("Terminal (text interface)", choose(launcher.ModeTUI))
	if defaultMode == launcher.ModeTUI {
		c.terminal.Importance = widget.HighImportance
	} else {
		c.dashboard.Importance = widget.HighImportance
	}
	c.quit = widget.NewButton("Quit", func() { done(launcher.PromptResult{Quit: true}) })
	title := heading("How do you want to open " + meta.AppName + "?")
	title.Wrapping = fyne.TextWrapWord
	c.icon = appIconImage(48)
	c.content = container.NewPadded(container.NewVBox(container.NewBorder(nil, nil, c.icon, nil, title), c.dashboard, c.terminal, c.remember, container.NewHBox(layout.NewSpacer(), c.quit),
		muted("Change this later in Settings, Open with.")))
	return c
}

func showChooser(window fyne.Window, defaultMode string, done func(launcher.PromptResult)) {
	c := newChooser(defaultMode, done)
	window.SetTitle("How do you want to open " + meta.AppName + "?")
	window.SetMainMenu(nil)
	window.SetContent(c.content)
	window.SetCloseIntercept(func() { done(launcher.PromptResult{Quit: true}) })
	window.Resize(fyne.NewSize(460, 290))
	window.SetFixedSize(true)
	window.CenterOnScreen()
	window.Show()
	if defaultMode == launcher.ModeTUI {
		window.Canvas().Focus(c.terminal)
	} else {
		window.Canvas().Focus(c.dashboard)
	}
}
