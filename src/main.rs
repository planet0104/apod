#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{anyhow, Result};
use arboard::Clipboard;
use chrono::{DateTime, Utc};
use image::EncodableLayout;
use std::collections::HashMap;
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem},
    TrayIcon, TrayIconBuilder,
};

const MENU_ID_PREV: &str = "1001";
const MENU_ID_NEXT: &str = "1002";
const MENU_ID_RAND: &str = "1003";
const MENU_ID_TODAY: &str = "1004";
const MENU_ID_DATE: &str = "1005";
const MENU_ID_HD: &str = "1006";
const MENU_ID_EXIT: &str = "2005";

#[derive(Clone)]
struct DownloadInfo {
    date: DateTime<Utc>,
    title: String,
    hd: bool,
}

fn main() -> Result<()> {
    let event_loop = EventLoopBuilder::<HashMap<String, String>>::with_user_event().build();

    let icon = load_tray_icon()?;

    let tray_icon = TrayIconBuilder::new()
        .with_tooltip("Astronomy Picture of the Day")
        .with_icon(icon)
        .build()?;

    let mut info = DownloadInfo {
        date: Utc::now(),
        title: "正在下载...".to_string(),
        hd: false,
    };

    let proxy = event_loop.create_proxy();

    tray_icon.set_menu(Some(Box::new(create_menu(&info)?)));

    download_picture_async(info.clone(), proxy.clone());

    let menu_channel = MenuEvent::receiver();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Ok(MenuEvent { id }) = menu_channel.try_recv() {
            if id == MENU_ID_EXIT {
                *control_flow = ControlFlow::Exit;
            } else {
                on_menu_event(id, &mut info, &tray_icon, proxy.clone());
            }
        }
        if let Event::UserEvent(event) = event {
            //更新菜单
            if let (Some(image_date), Some(image_title)) = (event.get("date"), event.get("title")) {
                info.date = image_date.parse().unwrap_or(info.date);
                info.title = format!("{image_title} - {image_date}");
                if let Ok(menu) = create_menu(&info) {
                    tray_icon.set_menu(Some(Box::new(menu)));
                }
            }
        }
    });
}

// 菜单点击处理
fn on_menu_event(
    id: MenuId,
    info: &mut DownloadInfo,
    tray_icon: &TrayIcon,
    proxy: EventLoopProxy<HashMap<String, String>>,
) {
    if id == MENU_ID_PREV {
        info.date = info.date - chrono::Duration::days(1);
        download_picture_async(info.clone(), proxy);
    } else if id == MENU_ID_NEXT {
        if info.date.format("%Y-%m-%d").to_string() == Utc::now().format("%Y-%m-%d").to_string() {
            return;
        }
        info.date = info.date + chrono::Duration::days(1);
        download_picture_async(info.clone(), proxy);
    } else if id == MENU_ID_TODAY {
        info.date = Utc::now();
        download_picture_async(info.clone(), proxy);
    } else if id == MENU_ID_RAND {
        let date1 = Utc::now() - chrono::Duration::days(180);
        let days = fastrand::i64(0..180);
        info.date = date1 + chrono::Duration::days(days);
        download_picture_async(info.clone(), proxy);
    } else if id == MENU_ID_DATE {
        //图片标题复制到剪贴板
        if let (true, Ok(mut clipboard)) = (info.title.len() > 0, Clipboard::new()) {
            let _ = clipboard.set_text(&info.title);
        }
    } else if id == MENU_ID_HD {
        //切换高清和非高清
        info.hd = !info.hd;
        if let Ok(menu) = create_menu(info) {
            tray_icon.set_menu(Some(Box::new(menu)));
        }
        download_picture_async(info.clone(), proxy);
    }
}

// 异步下载图片
fn download_picture_async(info: DownloadInfo, proxy: EventLoopProxy<HashMap<String, String>>) {
    std::thread::spawn(move || match download_picture(info) {
        Ok(json) => {
            println!("图片下载成功:{:?}", json.get("url"));
            let _ = proxy.send_event(json);
        }
        Err(err) => {
            eprintln!("图片下载失败:{:?}", err);
            let _ = proxy.send_event(HashMap::new());
        }
    });
}

// 下载壁纸
fn download_picture(mut info: DownloadInfo) -> Result<HashMap<String, String>> {
    let api_key = "DEMO_KEY";
    let formatted_date = info.date.format("%Y-%m-%d").to_string();
    let url =
        format!("https://api.nasa.gov/planetary/apod?api_key={api_key}&date={formatted_date}");
    println!("url={url}");
    let resp = reqwest::blocking::get(url)?;
    let status_code = resp.status().as_u16();
    println!("status_code={status_code}");
    if status_code == 400 || status_code == 404 {
        //往前一天
        info.date = info.date - chrono::Duration::days(1);
        return download_picture(info);
    }

    let json = resp.json::<HashMap<String, String>>()?;

    let url_key = if info.hd { "hdurl" } else { "url" };
    let url = match json.get(url_key) {
        None => return Err(anyhow!("{url_key} 为空!")),
        Some(url) => url,
    };
    println!("下载:{url}...");
    wallpaper::set_from_url(url).map_err(|err| anyhow!("{:?}", err))?;

    //设置锁屏
    #[cfg(target_os = "windows")]
    {
        use windows::{core::HSTRING, Storage::StorageFile, System::UserProfile::LockScreen};
        //下载图片
        let mut file_name = match dirs::download_dir() {
            Some(p) => p,
            None => return Err(anyhow!("路径错误")),
        };
        file_name.push(format!("lock-screen-{}.jpg", info.date.format("%Y-%m-%d")));
        image::load_from_memory(reqwest::blocking::get(url)?.bytes()?.as_bytes())?
            .save(&file_name)?;
        if let Some(path) = file_name.to_str() {
            let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path))?.get()?;
            LockScreen::SetImageFileAsync(&file)?.get()?;
            println!("锁屏设置成功:{path}");
        }
    }

    Ok(json)
}

// 创建菜单
fn create_menu(info: &DownloadInfo) -> Result<Menu> {
    let menu = Menu::new();
    menu.append(&MenuItem::with_id(MENU_ID_DATE, &info.title, true, None))?;
    menu.append(&MenuItem::with_id(MENU_ID_PREV, "上一张", true, None))?;
    menu.append(&MenuItem::with_id(MENU_ID_NEXT, "下一张", true, None))?;
    menu.append(&MenuItem::with_id(MENU_ID_TODAY, "今日", true, None))?;
    menu.append(&MenuItem::with_id(MENU_ID_RAND, "随机", true, None))?;
    menu.append(&MenuItem::with_id(
        MENU_ID_HD,
        if info.hd {
            "下载高清图☑"
        } else {
            "下载高清图"
        },
        true,
        None,
    ))?;
    menu.append(&MenuItem::with_id(MENU_ID_EXIT, "退出", true, None))?;
    Ok(menu)
}

// 加载icon
fn load_tray_icon() -> Result<tray_icon::Icon> {
    let icon_png = include_bytes!("../favicon.ico");
    let image =
        image::load_from_memory_with_format(icon_png, image::ImageFormat::Ico)?.into_rgba8();
    let (width, height) = image.dimensions();
    let rgba = image.into_raw();
    Ok(tray_icon::Icon::from_rgba(rgba, width, height)?)
}
