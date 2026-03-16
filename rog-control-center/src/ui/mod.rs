pub mod setup_anime;
pub mod setup_aura;
pub mod setup_dashboard;
pub mod setup_fans;
pub mod setup_system;

use std::sync::{Arc, Mutex};

use config_traits::StdConfig;
use log::warn;
use rog_dbus::list_iface_blocking;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};

use crate::config::Config;
use crate::ui::setup_anime::setup_anime_page;
use crate::ui::setup_aura::setup_aura_page;
use crate::ui::setup_dashboard::{setup_dashboard_monitoring, setup_dashboard_page};
use crate::ui::setup_fans::setup_fan_curve_page;
use crate::ui::setup_system::{setup_system_page, setup_system_page_callbacks};
use crate::{AppSettingsPageData, MainWindow, SideBarEntry};

// this macro sets up:
// - a link from UI callback -> dbus proxy property
// - a link from dbus property signal -> UI state
// conv1 and conv2 are type conversion args
#[macro_export]
macro_rules! set_ui_callbacks {
    ($handle:ident, $data:ident($($conv1: tt)*),$proxy:ident.$proxy_fn:tt($($conv2: tt)*),$success:literal,$failed:literal) => {
        let handle_copy = $handle.as_weak();
        let proxy_copy = $proxy.clone();
        let data = $handle.global::<$data>();
        concat_idents::concat_idents!(on_set = on_cb_, $proxy_fn {
        data.on_set(move |value| {
            let proxy_copy = proxy_copy.clone();
            let handle_copy = handle_copy.clone();
            tokio::spawn(async move {
                concat_idents::concat_idents!(set = set_, $proxy_fn {
                show_toast(
                    format!($success, value).into(),
                    $failed.into(),
                    handle_copy,
                    proxy_copy.set(value $($conv2)*).await,
                );
                });
            });
            });
        });
        let handle_copy = $handle.as_weak();
        let proxy_copy = $proxy.clone();
        concat_idents::concat_idents!(receive = receive_, $proxy_fn, _changed {
        // spawn required since the while let never exits
        tokio::spawn(async move {
            let mut x = proxy_copy.receive().await;
            concat_idents::concat_idents!(set = set_, $proxy_fn {
            use futures_util::StreamExt;
            while let Some(e) = x.next().await {
                if let Ok(out) = e.get().await {
                    handle_copy.upgrade_in_event_loop(move |handle| {
                        handle.global::<$data>().set(out $($conv1)*);
                    }).ok();
                }
            }
            });
        });
        });
    };
}

pub fn show_toast(
    success: SharedString,
    fail: SharedString,
    handle: Weak<MainWindow>,
    result: zbus::Result<()>,
) {
    match result {
        Ok(_) => {
            slint::invoke_from_event_loop(move || handle.unwrap().invoke_show_toast(success)).ok()
        }
        Err(e) => slint::invoke_from_event_loop(move || {
            log::warn!("{fail}: {e}");
            handle.unwrap().invoke_show_toast(fail)
        })
        .ok(),
    };
}

pub fn setup_window(config: Arc<Mutex<Config>>) -> MainWindow {
    slint::set_xdg_app_id("rog-control-center")
        .map_err(|e| warn!("Couldn't set application ID: {e:?}"))
        .ok();
    let ui = MainWindow::new()
        .map_err(|e| warn!("Couldn't create main window: {e:?}"))
        .unwrap();
    ui.window()
        .show()
        .map_err(|e| warn!("Couldn't show main window: {e:?}"))
        .unwrap();

    let available = list_iface_blocking().unwrap_or_default();

    let has_platform = available.contains(&"xyz.ljones.Platform".to_string());
    let has_aura = available.contains(&"xyz.ljones.Aura".to_string());
    let has_anime = available.contains(&"xyz.ljones.Anime".to_string());
    let has_fans = available.contains(&"xyz.ljones.FanCurves".to_string());

    // Sidebar entries: Dashboard(0), System(1), Aura(2), AniMe(3), Fans(4), Settings(5), About(6)
    let entries = vec![
        SideBarEntry {
            label: "Dashboard".into(),
            section: "".into(),
            available: true,
        },
        SideBarEntry {
            label: "System Control".into(),
            section: "Main".into(),
            available: has_platform,
        },
        SideBarEntry {
            label: "Keyboard Aura".into(),
            section: "Playground".into(),
            available: has_aura,
        },
        SideBarEntry {
            label: "AniMe Matrix".into(),
            section: "".into(),
            available: has_anime,
        },
        SideBarEntry {
            label: "Fan Curves".into(),
            section: "Controls".into(),
            available: has_fans,
        },
        SideBarEntry {
            label: "App Settings".into(),
            section: "Settings".into(),
            available: true,
        },
        SideBarEntry {
            label: "About".into(),
            section: "".into(),
            available: true,
        },
    ];
    ui.set_sidebar_entries(ModelRc::new(VecModel::from(entries)));

    ui.on_exit_app(move || {
        slint::quit_event_loop().unwrap();
    });

    // Dashboard page (always available)
    setup_dashboard_page(&ui);
    setup_dashboard_monitoring(ui.as_weak());

    setup_app_settings_page(&ui, config.clone());
    if has_platform {
        setup_system_page(&ui, config.clone());
        setup_system_page_callbacks(&ui, config.clone());
    }
    if has_aura {
        setup_aura_page(&ui, config.clone());
    }
    if has_anime {
        setup_anime_page(&ui, config.clone());
    }
    if has_fans {
        setup_fan_curve_page(&ui, config);
    }

    ui
}

pub fn setup_app_settings_page(ui: &MainWindow, config: Arc<Mutex<Config>>) {
    let config_copy = config.clone();
    let global = ui.global::<AppSettingsPageData>();
    global.on_set_run_in_background(move |enable| {
        if let Ok(mut lock) = config_copy.try_lock() {
            lock.run_in_background = enable;
            lock.write();
        }
    });
    let config_copy = config.clone();
    global.on_set_startup_in_background(move |enable| {
        if let Ok(mut lock) = config_copy.try_lock() {
            lock.startup_in_background = enable;
            lock.write();
        }
    });
    let config_copy = config.clone();
    global.on_set_enable_tray_icon(move |enable| {
        if let Ok(mut lock) = config_copy.try_lock() {
            lock.enable_tray_icon = enable;
            lock.write();
        }
    });
    let config_copy = config.clone();
    global.on_set_enable_dgpu_notifications(move |enable| {
        if let Ok(mut lock) = config_copy.try_lock() {
            lock.notifications.enabled = enable;
            lock.write();
        }
    });

    if let Ok(lock) = config.try_lock() {
        global.set_run_in_background(lock.run_in_background);
        global.set_startup_in_background(lock.startup_in_background);
        global.set_enable_tray_icon(lock.enable_tray_icon);
        global.set_enable_dgpu_notifications(lock.notifications.enabled);
    }
}
