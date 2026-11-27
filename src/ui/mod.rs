use crate::{
    engine::{Engine, Snapshot},
    model::*,
    tray::Tray,
};
use anyhow::Result;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
slint::include_modules!();
fn name(s: Service) -> &'static str {
    match s {
        Service::Chatgpt => "ChatGPT",
        Service::Claude => "Claude Code",
        _ => s.name(),
    }
}
fn current(app: &MainWindow) -> Option<Service> {
    Service::from_id(&app.get_selected_service())
}
fn fill_form(app: &MainWindow, engine: &Engine) {
    if let Some(s) = current(app) {
        let snap = engine.snapshot();
        let c = &snap.config.services[&s];
        app.set_edit_enabled(c.enabled);
        app.set_edit_browser(match c.browser.as_str() {
            "chrome" => 1,
            "edge" => 2,
            _ => 0,
        });
        app.set_edit_budget(c.budget.clone().into());
        app.set_edit_key("".into());
        app.set_edit_org(c.organization.clone().into());
        app.set_edit_currency(if c.currency == "CNY" { 1 } else { 0 });
        app.set_api_service(s.api());
        app.set_service_help(if s==Service::Chatgpt{"Zaloguj konto ChatGPT w swojej przeglądarce. Poczekamy na potwierdzenie autoryzacji, a następnie pobierzemy dostępne limity Codex na tym koncie.".into()}else{s.help().into()});
    }
    render(app, &engine.snapshot());
}
fn config(app: &MainWindow, engine: &Engine, s: Service) -> ServiceConfig {
    let mut c = engine.snapshot().config.services[&s].clone();
    c.enabled = app.get_edit_enabled();
    c.budget = app.get_edit_budget().to_string().replace(',', ".");
    c.currency = if app.get_edit_currency() == 1 {
        "CNY"
    } else {
        "USD"
    }
    .into();
    c.organization = app.get_edit_org().trim().into();
    c.browser_auth = false;
    c.browser = match app.get_edit_browser() {
        1 => "chrome",
        2 => "edge",
        _ => "auto",
    }
    .into();
    c
}
fn render(app: &MainWindow, s: &Snapshot) {
    if current(app).is_some_and(|service| s.config.hidden_services.contains(&service)) {
        app.set_selected_service("".into());
        app.set_page("overview".into());
    }
    app.set_appearance(match s.config.appearance.as_str() {
        "light" => 1,
        "dark" => 2,
        _ => 0,
    });
    app.set_text_scale(s.config.text_scale.into());
    app.set_gemini_state(s.gemini_state as i32);
    let tiles = s
        .config
        .service_order
        .iter()
        .copied()
        .enumerate()
        .map(|(i, service)| {
            let m = s
                .metrics
                .iter()
                .filter(|m| m.service == service && s.config.services[&service].enabled)
                .find(|m| m.identity() == s.config.selected)
                .or_else(|| {
                    s.metrics
                        .iter()
                        .find(|m| m.service == service && s.config.services[&service].enabled)
                });
            let p = m
                .filter(|m| m.current(s.config.interval))
                .and_then(|m| m.display_percent(s.config.remaining()));
            ServiceTile {
                id: service.id().into(),
                position: i as i32,
                visible: !s.config.hidden_services.contains(&service),
                status: s.status.get(&service).cloned().unwrap_or_default().into(),
                name: name(service).into(),
                value: m
                    .map(|m| {
                        if !service.api() {
                            m.display_percent(s.config.remaining())
                                .map(|p| format!("{p:.0}%"))
                                .unwrap_or_else(|| m.value.clone())
                        } else {
                            m.value.clone()
                        }
                    })
                    .unwrap_or_else(|| "—".into())
                    .into(),
                detail: m
                    .map(|m| {
                        if m.current(s.config.interval) {
                            if !service.api() && m.percent.is_some() {
                                format!(
                                    "{} · {}",
                                    if s.config.show_remaining {
                                        "Pozostało"
                                    } else {
                                        "Wykorzystano"
                                    },
                                    m.label
                                )
                            } else {
                                m.label.clone()
                            }
                        } else {
                            "Dane nieaktualne".into()
                        }
                    })
                    .unwrap_or_else(|| {
                        if s.config.services[&service].enabled {
                            "Oczekiwanie na dane".into()
                        } else {
                            "Połącz konto".into()
                        }
                    })
                    .into(),
                progress: (p.unwrap_or(0.) / 100.) as f32,
                known: p.is_some(),
                online: m.map(|m| m.current(s.config.interval)).unwrap_or(false),
            }
        })
        .collect::<Vec<_>>();
    let visible: Vec<_> = tiles.iter().filter(|t| t.visible).cloned().collect();
    app.set_navigation_index(
        visible
            .iter()
            .position(|t| t.id == app.get_selected_service())
            .map(|i| i as i32)
            .map(|i| i + 1)
            .unwrap_or(0),
    );
    if !app.get_services().iter().eq(visible.iter().cloned())
        || app.get_navigation_titles().row_count() != visible.len() + 1
    {
        app.set_navigation_titles(ModelRc::new(VecModel::from(
            std::iter::once("Wybierz usługę".into())
                .chain(visible.iter().map(|t| t.name.clone()))
                .collect::<Vec<_>>(),
        )));
        app.set_services(ModelRc::new(VecModel::from(visible)));
    }
    if !app.get_all_services().iter().eq(tiles.iter().cloned()) {
        app.set_all_services(ModelRc::new(VecModel::from(tiles)));
    }
    app.set_notice(s.message.clone().into());
    app.set_auth_message(s.auth_message.clone().into());
    app.set_auth_code(s.auth_code.clone().into());
    app.set_auth_url(s.auth_url.clone().into());
    app.set_auth_busy(s.auth_busy);
    if let Some(service) = current(app) {
        app.set_page_title(name(service).into());
        app.set_service_status(s.status.get(&service).cloned().unwrap_or_default().into());
        app.set_service_busy(s.busy.contains(&service));
        app.set_page_subtitle(
            match service {
                Service::Chatgpt => "Limity Codex powiązane z kontem ChatGPT",
                Service::Deepseek => "Saldo i środki na koncie API",
                Service::Openai | Service::Anthropic => "Koszty organizacji i własny budżet",
                _ => "Limity konta i dostępne środki",
            }
            .into(),
        );
        app.set_account_label(
            if service == Service::Chatgpt {
                s.account_label.clone()
            } else {
                String::new()
            }
            .into(),
        );
        let rows = s
            .metrics
            .iter()
            .filter(|m| m.service == service && s.config.services[&service].enabled)
            .map(|m| QuotaItem {
                id: m.identity().into(),
                title: m.label.clone().into(),
                value: m.display_value(s.config.remaining()).into(),
                percentage: m.percentage_label(s.config.remaining()).into(),
                reset: m.reset.clone().into(),
                source: m.error.as_deref().unwrap_or(&m.source).into(),
                progress: (m.display_percent(s.config.remaining()).unwrap_or(0.) / 100.) as f32,
                known: m.percent.is_some(),
                current: m.current(s.config.interval),
                pinned: m.identity() == s.config.selected,
            })
            .collect::<Vec<_>>();
        app.set_quotas(ModelRc::new(VecModel::from(rows)));
    } else {
        app.set_page_title("Przegląd".into());
        app.set_page_subtitle("Twoje usługi. Wszystkie limity pod ręką.".into());
    }
}
fn update_layout(app: &MainWindow) {
    let width = app.window().size().width as f32 / app.window().scale_factor();
    app.set_wide(width >= 832. * app.get_text_scale() as f32 / 100.);
}
pub fn run(engine: Engine, hidden: bool) -> Result<()> {
    let app = MainWindow::new()?;
    let tray = Rc::new(RefCell::new(Tray::start(engine.clone())));
    let available = tray.borrow().available;
    let s = engine.snapshot();
    app.set_interval_text(s.config.interval.to_string().into());
    app.set_edit_hidden(s.config.hidden);
    app.set_edit_show_remaining(s.config.show_remaining);
    app.set_edit_show_api_remaining(s.config.show_api_remaining);
    render(&app, &s);
    let e = engine.clone();
    let w = app.as_weak();
    app.on_service_changed(move |_| {
        if let Some(a) = w.upgrade() {
            fill_form(&a, &e);
        }
    });
    let e = engine.clone();
    let w = app.as_weak();
    app.on_navigation_changed(move |index| {
        if let Some(a) = w.upgrade() {
            if let Some(item) = index
                .checked_sub(1)
                .and_then(|i| usize::try_from(i).ok())
                .and_then(|i| a.get_services().row_data(i))
            {
                a.set_selected_service(item.id);
                a.set_page("service".into());
                a.set_settings_open(false);
                fill_form(&a, &e);
            }
        }
    });
    let e = engine.clone();
    let w = app.as_weak();
    app.on_visibility_requested(move |id, visible| {
        if let Some(s) = Service::from_id(&id) {
            e.visibility(s, visible);
            if let Some(a) = w.upgrade() {
                update_layout(&a);
                render(&a, &e.snapshot());
            }
        }
    });
    let e = engine.clone();
    let w = app.as_weak();
    app.on_move_requested(move |id, position| {
        if let Some(s) = Service::from_id(&id) {
            e.reorder(s, position.clamp(0, 5) as usize);
            if let Some(a) = w.upgrade() {
                render(&a, &e.snapshot());
                let w = a.as_weak();
                slint::Timer::single_shot(Duration::from_millis(1), move || {
                    if let Some(a) = w.upgrade() {
                        a.set_move_focus_id(id);
                        a.set_move_focus_generation(a.get_move_focus_generation() + 1);
                    }
                });
            }
        }
    });
    let e = engine.clone();
    let w = app.as_weak();
    app.on_reset_layout_requested(move || {
        e.reset_layout();
        if let Some(a) = w.upgrade() {
            render(&a, &e.snapshot());
        }
    });
    let e = engine.clone();
    let w = app.as_weak();
    app.on_appearance_requested(move || {
        if let Some(a) = w.upgrade() {
            e.appearance(
                match a.get_appearance() {
                    1 => "light",
                    2 => "dark",
                    _ => "system",
                }
                .into(),
                a.get_text_scale() as u16,
            );
            render(&a, &e.snapshot());
        }
    });
    let e = engine.clone();
    app.on_continue_gemini_requested(move || e.continue_gemini());
    let e = engine.clone();
    app.on_cancel_gemini_requested(move || e.cancel_gemini());
    let e = engine.clone();
    let w = app.as_weak();
    app.on_save_requested(move || {
        if let Some(a) = w.upgrade() {
            if let Some(s) = current(&a) {
                let key = a.get_edit_key().to_string();
                a.set_edit_key("".into());
                e.save(s, config(&a, &e, s), key, a.get_edit_remember());
            }
        }
    });
    let e = engine.clone();
    let w = app.as_weak();
    app.on_login_requested(move |device| {
        if let Some(a) = w.upgrade() {
            if let Some(s) = current(&a) {
                a.set_edit_enabled(true);
                let mut c = config(&a, &e, s);
                c.device_login = device;
                e.login_config(s, c);
            }
        }
    });
    let e = engine.clone();
    let w = app.as_weak();
    app.on_logout_requested(move || {
        if let Some(a) = w.upgrade() {
            if let Some(s) = current(&a) {
                e.logout(s);
            }
        }
    });
    let e = engine.clone();
    let w = app.as_weak();
    app.on_delete_key_requested(move || {
        if let Some(a) = w.upgrade() {
            if let Some(s) = current(&a) {
                e.delete_key(s);
            }
        }
    });
    let e = engine.clone();
    app.on_refresh_requested(move || e.refresh_all());
    let e = engine.clone();
    app.on_pin_requested(move |id| e.pin(&id));
    let e = engine.clone();
    let w = app.as_weak();
    app.on_general_save_requested(move || {
        if let Some(a) = w.upgrade() {
            match a.get_interval_text().parse() {
                Ok(n) => e.general(
                    n,
                    a.get_edit_hidden(),
                    a.get_edit_show_remaining(),
                    a.get_edit_show_api_remaining(),
                ),
                Err(_) => e.message("Wpisz poprawną liczbę sekund (60–3600)"),
            }
        }
    });
    let e = engine.clone();
    let w = app.as_weak();
    app.on_hide_requested(move || {
        if available {
            if let Some(a) = w.upgrade() {
                let _ = a.hide();
            }
        } else {
            e.message(
                "Brak obsługi tray. W GNOME włącz rozszerzenie AppIndicator/KStatusNotifierItem.",
            );
        }
    });
    let e = engine.clone();
    app.on_quit_requested(move || e.quit());
    let e = engine.clone();
    app.on_cancel_auth_requested(move || e.cancel_login());
    let e = engine.clone();
    app.on_open_auth_requested(move || {
        let s = e.snapshot();
        if !s.auth_url.is_empty() {
            let _ = open::that(s.auth_url);
        }
    });
    let e = engine.clone();
    app.window().on_close_requested(move || {
        if available {
            slint::CloseRequestResponse::HideWindow
        } else {
            e.message(
                "Nie można schować okna bez obsługi tray. Użyj Zamknij aplikację, aby zakończyć.",
            );
            slint::CloseRequestResponse::KeepWindowShown
        }
    });
    let last = Rc::new(Cell::new((0, 0)));
    let tray_tick = tray.clone();
    let e = engine.clone();
    let w = app.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(300),
        move || {
            e.tick();
            if let Some(a) = w.upgrade() {
                update_layout(&a);
                if e.take_show() {
                    let _ = a.show();
                }
                let s = e.snapshot();
                let marker = (s.revision, chrono::Utc::now().timestamp() / 15);
                if last.get() != marker {
                    last.set(marker);
                    render(&a, &s);
                    tray_tick.borrow().update();
                }
                if e.quitting() {
                    let _ = slint::quit_event_loop();
                }
            }
        },
    );
    if !(hidden || s.config.hidden) || !available {
        app.show()?;
    }
    let visual_timer = slint::Timer::default();
    let visual_error = Rc::new(RefCell::new(None));
    let args: Vec<_> = std::env::args().collect();
    if let Some(path) = args
        .windows(2)
        .find(|a| a[0] == "--visual-test")
        .map(|a| a[1].clone())
    {
        let value = |flag: &str| args.windows(2).find(|a| a[0] == flag).map(|a| a[1].clone());
        let width = value("--width")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(960.);
        let height = value("--height")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(680.);
        app.window()
            .set_size(slint::LogicalSize::new(width, height));
        if let Some(view) = value("--view") {
            if let Some(service) = Service::from_id(&view) {
                app.set_selected_service(service.id().into());
                app.set_page("service".into());
                fill_form(&app, &engine);
                app.set_settings_open(args.iter().any(|v| v == "--account-settings"));
            } else {
                app.set_page(view.into());
            }
        }
        update_layout(&app);
        if let Some(amount) = value("--scroll").and_then(|v| v.parse::<f32>().ok()) {
            let w = app.as_weak();
            slint::Timer::single_shot(Duration::from_millis(500), move || {
                if let Some(a) = w.upgrade() {
                    a.window()
                        .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                            position: slint::LogicalPosition::new(width - 100., height / 2.),
                            delta_x: 0.,
                            delta_y: -amount,
                        });
                }
            });
        }
        let interaction_test = args.iter().any(|v| v == "--drag-test");
        let controls_test = args.iter().any(|v| v == "--controls-test");
        if controls_test {
            let w = app.as_weak();
            slint::Timer::single_shot(Duration::from_millis(300), move || {
                if let Some(a) = w.upgrade() {
                    use slint::platform::{PointerEventButton, WindowEvent};
                    let click = |x, y| {
                        a.window().dispatch_event(WindowEvent::PointerPressed {
                            position: slint::LogicalPosition::new(x, y),
                            button: PointerEventButton::Left,
                        });
                        a.window().dispatch_event(WindowEvent::PointerReleased {
                            position: slint::LogicalPosition::new(x, y),
                            button: PointerEventButton::Left,
                        });
                    };
                    click(272., 213.);
                    click(510., 250.);
                }
            });
        }
        if interaction_test {
            let w = app.as_weak();
            slint::Timer::single_shot(Duration::from_millis(300), move || {
                if let Some(a) = w.upgrade() {
                    use slint::platform::{PointerEventButton, WindowEvent};
                    a.window().dispatch_event(WindowEvent::PointerPressed {
                        position: slint::LogicalPosition::new(280., 175.),
                        button: PointerEventButton::Left,
                    });
                    let w = a.as_weak();
                    slint::Timer::single_shot(Duration::from_millis(100), move || {
                        if let Some(a) = w.upgrade() {
                            a.window().dispatch_event(WindowEvent::PointerMoved {
                                position: slint::LogicalPosition::new(280., 338.),
                            });
                            a.window().dispatch_event(WindowEvent::PointerReleased {
                                position: slint::LogicalPosition::new(280., 338.),
                                button: PointerEventButton::Left,
                            });
                        }
                    });
                }
            });
        }
        let w = app.as_weak();
        let e = engine.clone();
        let error_out = visual_error.clone();
        visual_timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_millis(1200),
            move || {
                let result = (|| -> Result<()> {
                    let a = w
                        .upgrade()
                        .ok_or_else(|| anyhow::anyhow!("Window closed"))?;
                    if controls_test {
                        anyhow::ensure!(
                            e.snapshot()
                                .config
                                .hidden_services
                                .contains(&Service::Chatgpt),
                            "Checkbox did not hide the service"
                        );
                        anyhow::ensure!(
                            !a.get_services().iter().any(|item| item.id == "chatgpt"),
                            "Hidden service is still in navigation"
                        );
                    }
                    if interaction_test || controls_test {
                        anyhow::ensure!(
                            e.snapshot().config.service_order[1] == Service::Chatgpt,
                            "Drag did not move ChatGPT to position 2"
                        );
                        use slint::platform::{Key, WindowEvent};
                        a.window().dispatch_event(WindowEvent::KeyPressed {
                            text: Key::DownArrow.into(),
                        });
                        a.window().dispatch_event(WindowEvent::KeyReleased {
                            text: Key::DownArrow.into(),
                        });
                        anyhow::ensure!(
                            e.snapshot().config.service_order[2] == Service::Chatgpt,
                            "Keyboard focus did not follow reordered service"
                        );
                    }
                    let image = a.window().take_snapshot()?;
                    let mut bytes =
                        format!("P6\n{} {}\n255\n", image.width(), image.height()).into_bytes();
                    for pixel in image.as_bytes().chunks_exact(4) {
                        bytes.extend_from_slice(&pixel[..3]);
                    }
                    std::fs::write(&path, bytes)?;
                    Ok(())
                })();
                if let Err(error) = result {
                    eprintln!("Screenshot failed: {error:#}");
                    *error_out.borrow_mut() = Some(error);
                }
                e.quit();
            },
        );
    }
    slint::run_event_loop_until_quit()?;
    engine.shutdown();
    tray.borrow_mut().stop();
    if let Some(error) = visual_error.borrow_mut().take() {
        return Err(error);
    }
    Ok(())
}
pub fn error(message: &str) {
    #[cfg(windows)]
    {
        let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
        let title: Vec<u16> = "AI Gauge".encode_utf16().chain(Some(0)).collect();
        unsafe {
            winapi::um::winuser::MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                title.as_ptr(),
                winapi::um::winuser::MB_OK | winapi::um::winuser::MB_ICONERROR,
            );
        }
    }
    #[cfg(not(windows))]
    eprintln!("AI Gauge: {message}");
}
