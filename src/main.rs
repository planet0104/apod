#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem},
    TrayIconBuilder,
};

const MENU_ID_PREV: &str = "1001";
const MENU_ID_NEXT: &str = "1002";
const MENU_ID_RAND: &str = "1003";
const MENU_ID_TODAY: &str = "1004";
const MENU_ID_EXIT: &str = "2005";

fn main() -> Result<()> {
    let event_loop = EventLoopBuilder::new().build();

    let menu = create_menu()?;
    let icon = load_tray_icon()?;

    let _tray_icon = Some(
        TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Astronomy Picture of the Day")
            .with_icon(icon)
            .build()?,
    );

    let mut date = Utc::now();

    download_picture_async(date);

    let menu_channel = MenuEvent::receiver();

    event_loop.run(move |_event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Ok(MenuEvent { id }) = menu_channel.try_recv() {
            if id == MENU_ID_EXIT {
                *control_flow = ControlFlow::Exit;
            } else {
                on_menu_event(id, &mut date);
            }
        }
    });
}

// 菜单点击处理
fn on_menu_event(id: MenuId, current_date: &mut DateTime<Utc>) {
    if id == MENU_ID_PREV {
        *current_date = *current_date - chrono::Duration::days(1);
        download_picture_async(*current_date);
    } else if id == MENU_ID_NEXT {
        if *current_date.format("%Y-%m-%d").to_string() == Utc::now().format("%Y-%m-%d").to_string()
        {
            return;
        }
        *current_date = *current_date + chrono::Duration::days(1);
        download_picture_async(*current_date);
    } else if id == MENU_ID_TODAY {
        *current_date = Utc::now();
        download_picture_async(*current_date);
    } else if id == MENU_ID_RAND {
        let date1 = Utc::now() - chrono::Duration::days(180);
        let days = fastrand::i64(0..180);
        *current_date = date1 + chrono::Duration::days(days);
        download_picture_async(*current_date);
    }
}

// 异步下载图片
fn download_picture_async(date: DateTime<Utc>) {
    std::thread::spawn(move || match download_picture(date) {
        Ok(url) => println!("图片下载成功:{url}"),
        Err(err) => eprintln!("图片下载失败:{:?}", err),
    });
}

// 下载壁纸
fn download_picture(date: DateTime<Utc>) -> Result<String> {
    //api示例: https://api.nasa.gov/planetary/apod?api_key=DEMO_KEY
    // let api_key = std::env::var("NASA_API_KEY")?;
    let api_key = "DEMO_KEY";
    let formatted_date = date.format("%Y-%m-%d").to_string();
    let json = reqwest::blocking::get(format!(
        "https://api.nasa.gov/planetary/apod?api_key={api_key}&date={formatted_date}"
    ))?
    .json::<HashMap<String, String>>()?;
    // 下载地址存放于url和hdurl, hdurl是高清图
    let url = match json.get("url") {
        None => return Err(anyhow!("url 为空!")),
        Some(url) => url,
    };
    println!("下载:{url}...");
    wallpaper::set_from_url(url).map_err(|err| anyhow!("{:?}", err))?;
    Ok(url.to_string())
}

// 创建菜单
fn create_menu() -> Result<Menu> {
    let menu = Menu::new();
    menu.append(&MenuItem::with_id(MENU_ID_PREV, "上一张", true, None))?;
    menu.append(&MenuItem::with_id(MENU_ID_NEXT, "下一张", true, None))?;
    menu.append(&MenuItem::with_id(MENU_ID_TODAY, "今日", true, None))?;
    menu.append(&MenuItem::with_id(MENU_ID_RAND, "随机", true, None))?;
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
