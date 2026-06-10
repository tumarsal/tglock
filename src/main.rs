mod proxy;

use clap::Parser;
use std::sync::atomic::Ordering;
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "tglock",
    version,
    about = "Обход блокировки Telegram через локальный WebSocket-туннель"
)]
struct Cli {
    /// Порт SOCKS5-прокси
    #[arg(short, long, default_value_t = proxy::DEFAULT_PORT)]
    port: u16,

    /// Слушать на 0.0.0.0 (LAN-режим)
    #[arg(long)]
    lan: bool,

    /// Периодически выводить статистику соединений
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let stats = proxy::Stats::new();

    let addr = if cli.lan { "0.0.0.0" } else { "127.0.0.1" };
    eprintln!("Запускаю SOCKS5 на {}:{}...", addr, cli.port);

    let stats_run = stats.clone();
    let lan = cli.lan;
    let port = cli.port;
    let mut proxy_task = tokio::spawn(async move { proxy::run(stats_run, lan, port).await });

    // Даём прокси время подняться
    tokio::time::sleep(Duration::from_millis(250)).await;

    if !stats.running.load(Ordering::SeqCst) {
        match proxy_task.await {
            Ok(Err(e)) => return Err(e.into()),
            Ok(Ok(())) => return Err("не удалось запустить прокси".into()),
            Err(e) => return Err(e.into()),
        }
    }

    print_banner(&cli);

    if cli.verbose {
        let stats_log = stats.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                if !stats_log.running.load(Ordering::SeqCst) {
                    break;
                }
                let active = stats_log.active.load(Ordering::Relaxed);
                let total = stats_log.total.load(Ordering::Relaxed);
                let ws = stats_log.ws.load(Ordering::Relaxed);
                let dc = stats_log.last_dc.load(Ordering::Relaxed);
                let dc_str = if dc > 0 { dc.to_string() } else { "—".into() };
                eprintln!(
                    "[stats] соединения={} ws={} dc={} всего={}",
                    active, ws, dc_str, total
                );
            }
        });
    }

    eprintln!("Работает. Нажмите Ctrl+C для остановки.");

    tokio::select! {
        res = &mut proxy_task => {
            res??;
        }
        _ = tokio::signal::ctrl_c() => {
            eprintln!("\nОстанавливаю...");
            stats.running.store(false, Ordering::SeqCst);
            proxy_task.await??;
        }
    }

    eprintln!("Остановлен.");
    Ok(())
}

fn print_banner(cli: &Cli) {
    let server = if cli.lan {
        local_ip().unwrap_or_else(|| "127.0.0.1".into())
    } else {
        "127.0.0.1".into()
    };

    eprintln!();
    eprintln!("SOCKS5: {}:{}", server, cli.port);
    if cli.lan {
        eprintln!("LAN-режим: другие устройства в сети могут подключаться");
    }
    eprintln!();
    eprintln!("Настройка Telegram:");
    eprintln!("  Настройки → Продвинутые → Тип соединения → SOCKS5");
    eprintln!("  Сервер: {}", server);
    eprintln!("  Порт:   {}", cli.port);
    eprintln!();
    eprintln!(
        "Автонастройка: tg://socks?server={}&port={}",
        server, cli.port
    );
    eprintln!();
}

fn local_ip() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}
