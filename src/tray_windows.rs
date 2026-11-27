use crate::{
    engine::{self, Engine},
    icon,
};
use anyhow::Result;
use native_windows_gui as nwg;
use std::{cell::RefCell, rc::Rc, sync::mpsc, thread::JoinHandle, time::Duration};
#[derive(Clone)]
enum Action {
    Show,
    Refresh,
    Pin(String),
    Quit,
}
struct Popup {
    root: nwg::Menu,
    _sub: Vec<nwg::Menu>,
    items: Vec<(nwg::MenuItem, Action)>,
}
struct NativeTray {
    engine: Engine,
    window: nwg::MessageWindow,
    tray: nwg::TrayNotification,
    timer: nwg::AnimationTimer,
    icon: RefCell<nwg::Icon>,
    popup: RefCell<Option<Popup>>,
    last: RefCell<(String, (u32, u32))>,
    handlers: RefCell<Vec<nwg::EventHandler>>,
}
impl NativeTray {
    fn build(engine: Engine) -> Result<Rc<Self>> {
        nwg::init()?;
        let mut window = nwg::MessageWindow::default();
        nwg::MessageWindow::builder().build(&mut window)?;
        let path = engine.data_dir().join("tray.ico");
        std::fs::write(&path, icon::ico(None))?;
        let icon = load_icon(&path, icon_size())?;
        let mut tray = Default::default();
        nwg::TrayNotification::builder()
            .parent(&window)
            .icon(Some(&icon))
            .tip(Some("AI Gauge"))
            .build(&mut tray)?;
        let mut timer = Default::default();
        nwg::AnimationTimer::builder()
            .parent(&window)
            .interval(Duration::from_millis(500))
            .active(true)
            .build(&mut timer)?;
        let ui = Rc::new(Self {
            engine,
            window,
            tray,
            timer,
            icon: RefCell::new(icon),
            popup: RefCell::new(None),
            last: RefCell::new((String::new(), (0, 0))),
            handlers: RefCell::new(vec![]),
        });
        let weak = Rc::downgrade(&ui);
        let handler = nwg::full_bind_event_handler(&ui.window.handle, move |e, _, handle| {
            if let Some(ui) = weak.upgrade() {
                use nwg::Event as E;
                match e {
                    E::OnTimerTick if handle == ui.timer.handle => {
                        ui.update();
                        if ui.engine.quitting() {
                            nwg::stop_thread_dispatch();
                        }
                    }
                    E::OnContextMenu if handle == ui.tray.handle => {
                        if let Ok(p) = ui.menu() {
                            *ui.popup.borrow_mut() = Some(p);
                            let (x, y) = nwg::GlobalCursor::position();
                            if let Some(p) = ui.popup.borrow().as_ref() {
                                p.root.popup(x, y);
                            }
                        }
                    }
                    E::OnMousePress(nwg::MousePressEvent::MousePressLeftUp)
                        if handle == ui.tray.handle =>
                    {
                        ui.engine.request_show()
                    }
                    E::OnMenuItemSelected => {
                        let action = ui.popup.borrow().as_ref().and_then(|p| {
                            p.items
                                .iter()
                                .find(|(i, _)| i.handle == handle)
                                .map(|(_, a)| a.clone())
                        });
                        match action {
                            Some(Action::Show) => ui.engine.request_show(),
                            Some(Action::Refresh) => ui.engine.refresh_all(),
                            Some(Action::Pin(id)) => ui.engine.pin(&id),
                            Some(Action::Quit) => ui.engine.quit(),
                            None => {}
                        }
                    }
                    _ => {}
                }
            }
        });
        ui.handlers.borrow_mut().push(handler);
        ui.update();
        Ok(ui)
    }
    fn update(&self) {
        let s = self.engine.snapshot();
        let p = engine::tray_percent(&s);
        let marker = (icon::text(p), icon_size());
        if *self.last.borrow() != marker {
            let path = self.engine.data_dir().join("tray.ico");
            if std::fs::write(&path, icon::ico(p)).is_ok() {
                if let Ok(icon) = load_icon(&path, marker.1) {
                    self.tray.set_icon(&icon);
                    *self.icon.borrow_mut() = icon;
                    *self.last.borrow_mut() = marker;
                }
            }
        }
        self.tray
            .set_tip(&engine::tooltip(&s).chars().take(120).collect::<String>());
    }
    fn menu(&self) -> Result<Popup> {
        let s = self.engine.snapshot();
        let mut root = Default::default();
        nwg::Menu::builder()
            .parent(&self.window)
            .popup(true)
            .build(&mut root)?;
        let mut items = vec![];
        let mut sub = vec![];
        for (text, a) in [
            ("Pokaż AI Gauge", Action::Show),
            ("Odśwież wszystkie", Action::Refresh),
        ] {
            let mut i = Default::default();
            nwg::MenuItem::builder()
                .parent(&root)
                .text(text)
                .build(&mut i)?;
            items.push((i, a));
        }
        for service in s.config.service_order.iter().copied() {
            let mut menu = Default::default();
            nwg::Menu::builder()
                .parent(&root)
                .text(service.name())
                .build(&mut menu)?;
            let rows: Vec<_> = s
                .metrics
                .iter()
                .filter(|m| m.service == service && s.config.services[&service].enabled)
                .collect();
            if rows.is_empty() {
                let mut i = Default::default();
                nwg::MenuItem::builder()
                    .parent(&menu)
                    .text(
                        s.status
                            .get(&service)
                            .map(String::as_str)
                            .unwrap_or("Brak danych"),
                    )
                    .build(&mut i)?;
                i.set_enabled(false);
                items.push((i, Action::Show));
            }
            for m in rows {
                let mut i = Default::default();
                nwg::MenuItem::builder()
                    .parent(&menu)
                    .text(&format!(
                        "{} · {}",
                        m.line(s.config.interval, s.config.remaining()),
                        m.reset
                    ))
                    .build(&mut i)?;
                i.set_enabled(m.percent.is_some());
                i.set_checked(m.identity() == s.config.selected);
                items.push((i, Action::Pin(m.identity())));
            }
            sub.push(menu);
        }
        let mut i = Default::default();
        nwg::MenuItem::builder()
            .parent(&root)
            .text("Zamknij aplikację")
            .build(&mut i)?;
        items.push((i, Action::Quit));
        Ok(Popup {
            root,
            _sub: sub,
            items,
        })
    }
}
fn icon_size() -> (u32, u32) {
    use winapi::um::winuser::{GetSystemMetrics, SM_CXSMICON, SM_CYSMICON};
    unsafe {
        (
            GetSystemMetrics(SM_CXSMICON).max(16) as u32,
            GetSystemMetrics(SM_CYSMICON).max(16) as u32,
        )
    }
}
fn load_icon(path: &std::path::Path, size: (u32, u32)) -> Result<nwg::Icon> {
    let mut icon = nwg::Icon::default();
    nwg::Icon::builder()
        .source_file(Some(&path.to_string_lossy()))
        .size(Some(size))
        .strict(true)
        .build(&mut icon)?;
    Ok(icon)
}
pub struct Tray {
    pub available: bool,
    thread: Option<JoinHandle<()>>,
}
impl Tray {
    pub fn start(engine: Engine) -> Self {
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || match NativeTray::build(engine) {
            Ok(ui) => {
                let _ = tx.send(true);
                nwg::dispatch_thread_events();
                for h in ui.handlers.borrow_mut().drain(..) {
                    nwg::unbind_event_handler(&h);
                }
            }
            Err(_) => {
                let _ = tx.send(false);
            }
        });
        Self {
            available: rx.recv_timeout(Duration::from_secs(5)).unwrap_or(false),
            thread: Some(handle),
        }
    }
    pub fn update(&self) {}
    pub fn stop(&mut self) {
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
