use crate::{
    engine::{self, Engine},
    icon,
};
use ksni::blocking::TrayMethods;
struct LinuxTray {
    engine: Engine,
}
impl ksni::Tray for LinuxTray {
    fn id(&self) -> String {
        "ai-gauge".into()
    }
    fn title(&self) -> String {
        "AI Gauge".into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let s = self.engine.snapshot();
        let p = engine::tray_percent(&s);
        icon::SIZES
            .into_iter()
            .map(|size| ksni::Icon {
                width: size as i32,
                height: size as i32,
                data: icon::argb(p, size),
            })
            .collect()
    }
    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "AI Gauge".into(),
            description: engine::tooltip(&self.engine.snapshot()),
            ..Default::default()
        }
    }
    fn activate(&mut self, _x: i32, _y: i32) {
        self.engine.request_show();
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        let snap = self.engine.snapshot();
        let mut result = vec![
            StandardItem {
                label: "Pokaż AI Gauge".into(),
                activate: Box::new(|t: &mut Self| t.engine.request_show()),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Odśwież wszystkie".into(),
                activate: Box::new(|t: &mut Self| t.engine.refresh_all()),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
        ];
        for s in snap.config.service_order.iter().copied() {
            let mut items = vec![];
            for m in snap
                .metrics
                .iter()
                .filter(|m| m.service == s && snap.config.services[&s].enabled)
            {
                let id = m.identity();
                items.push(
                    CheckmarkItem {
                        label: format!(
                            "{}{}",
                            m.line(snap.config.interval, snap.config.remaining()),
                            if m.reset.is_empty() {
                                String::new()
                            } else {
                                format!(" · {}", m.reset)
                            }
                        ),
                        checked: id == snap.config.selected,
                        enabled: m.percent.is_some(),
                        activate: Box::new(move |t: &mut Self| t.engine.pin(&id)),
                        ..Default::default()
                    }
                    .into(),
                );
            }
            if items.is_empty() {
                items.push(
                    StandardItem {
                        label: snap
                            .status
                            .get(&s)
                            .cloned()
                            .unwrap_or_else(|| "Brak danych".into()),
                        enabled: false,
                        ..Default::default()
                    }
                    .into(),
                )
            }
            result.push(
                SubMenu {
                    label: s.name().into(),
                    submenu: items,
                    ..Default::default()
                }
                .into(),
            );
        }
        result.push(ksni::MenuItem::Separator);
        result.push(
            StandardItem {
                label: "Zamknij aplikację".into(),
                activate: Box::new(|t: &mut Self| t.engine.quit()),
                ..Default::default()
            }
            .into(),
        );
        result
    }
}

pub struct Tray {
    pub available: bool,
    handle: Option<ksni::blocking::Handle<LinuxTray>>,
}
impl Tray {
    pub fn start(engine: Engine) -> Self {
        let handle = LinuxTray { engine }.spawn().ok();
        Self {
            available: handle.is_some(),
            handle,
        }
    }
    pub fn update(&self) {
        if let Some(h) = &self.handle {
            h.update(|_| {});
        }
    }
    pub fn stop(&mut self) {
        self.handle.take();
    }
}
