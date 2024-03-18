use clap::*;
use std::str::FromStr;
use std::sync::{atomic::*, *};

mod elo;
mod engine;
mod pgn;

#[derive(Debug, Parser)]
struct Args {
    a: String,
    b: String,

    time: usize,
    inc: usize,

    #[arg(long, default_value = "openings.txt")]
    opening_positions: String,

    #[arg(long, action = ArgAction::SetTrue)]
    biased: bool,

    #[arg(short = 'n', long, default_value_t = usize::MAX)]
    play_positions: usize,

    #[arg(short, long, default_value_t = 1)]
    jobs: usize,

    #[arg(long, default_value_t = 1200.0)]
    a_elo: f32,
    #[arg(long, default_value_t = 1200.0)]
    b_elo: f32,
}

static THREADS: AtomicUsize = AtomicUsize::new(0);

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let game_result = [
        AtomicUsize::new(0),
        AtomicUsize::new(0),
        AtomicUsize::new(0),
    ]; // a win | draw | b win

    let fens = std::fs::read_to_string(args.opening_positions)
        .unwrap()
        .trim()
        .lines()
        .take(args.play_positions)
        .map(|fen| (chess::Game::from_str(fen).unwrap(), fen.to_string()))
        .collect::<Vec<(chess::Game, String)>>();

    let a_player = Arc::new(Player {
        path: args.a.as_str().into(),
        name: engine::Engine::get_name(args.a.as_str())
            .map_or_else(|| args.a.as_str().into(), |a| a.as_str().into()),
        elo: Arc::new(Mutex::new(args.a_elo)),
    });
    let b_player = Arc::new(Player {
        path: args.b.as_str().into(),
        name: engine::Engine::get_name(args.b.as_str())
            .map_or_else(|| args.b.as_str().into(), |a| a.as_str().into()),
        elo: Arc::new(Mutex::new(args.b_elo)),
    });

    println!("\x1b[1;32mInfo:\x1b[0m initialization complete");

    for (game, fen) in fens.iter() {
        let fen = unsafe { core::mem::transmute::<_, &'static _>(fen.as_str()) };
        let game_result =
            unsafe { core::mem::transmute::<_, &'static [AtomicUsize; 3]>(&game_result) };

        play(
            Arc::clone(&a_player),
            Arc::clone(&b_player),
            game.clone(),
            fen,
            args.time,
            args.inc,
            game_result,
            false,
            args.jobs,
        )
        .await;

        if !args.biased {
            play(
                Arc::clone(&b_player),
                Arc::clone(&a_player),
                game.clone(),
                fen,
                args.time,
                args.inc,
                game_result,
                true,
                args.jobs,
            )
            .await;
        }
    }

    while THREADS.load(Ordering::Relaxed) != 0 {
        core::hint::spin_loop();
    }

    let total = fens.len() * (2 - args.biased as usize);

    let a = game_result[0].load(Ordering::Relaxed);
    let d = game_result[1].load(Ordering::Relaxed);
    let b = game_result[2].load(Ordering::Relaxed);

    let term_size = term_size::dimensions().unwrap_or((80, 24));
    let bar_length = term_size.0 - 4;

    let a_bar_length = ((a as f32 / total as f32) * bar_length as f32).round() as usize;
    let d_bar_length = ((d as f32 / total as f32) * bar_length as f32).round() as usize;
    let b_bar_length = bar_length - a_bar_length - d_bar_length;

    let a_bar = "━".repeat(a_bar_length);
    let d_bar = "━".repeat(d_bar_length);
    let b_bar = "━".repeat(b_bar_length);

    let _d_pad = a_bar_length as isize - a.checked_ilog10().unwrap_or(0) as isize - 1;
    let d_pad = _d_pad.max(1) as usize;
    let d_error = d_pad as isize - _d_pad;
    let b_pad = (d_bar_length as isize - d.checked_ilog10().unwrap_or(0) as isize - 1 - d_error)
        .max(1) as usize;

    let indents_length = term_size.0 - " SUMMARY ".len();
    let l_indent_length = indents_length / 2;
    let r_indent_length = indents_length - l_indent_length;

    let l_indent = "=".repeat(l_indent_length);
    let r_indent = "=".repeat(r_indent_length);

    println!("\n\n\x1b[1m{} SUMMARY {}\x1b[0m", l_indent, r_indent);
    println!("\x1b[1mTotal:\x1b[0m {total} games");
    println!("  \x1b[32m{a}\x1b[90m\x1b[{d_pad}C{d}\x1b[31m\x1b[{b_pad}C{b}\x1b[0m");
    println!("  \x1b[32m{a_bar}\x1b[90m{d_bar}\x1b[31m{b_bar}\x1b[0m");
    println!(
        "\n \x1b[1mElo:\x1b[0m A: {:.0}, B: {:.0}",
        a_player.elo.lock().unwrap(),
        b_player.elo.lock().unwrap()
    );
}

fn flip(idx: usize, flip: bool, n: usize) -> usize {
    if flip {
        n - idx
    } else {
        idx
    }
}

pub struct Player {
    pub path: Arc<str>,
    pub name: Arc<str>,
    pub elo: Arc<Mutex<f32>>,
}

async fn play(
    a: Arc<Player>,
    b: Arc<Player>,
    mut game: chess::Game,
    fen: &'static str,
    time: usize,
    inc: usize,
    game_result: &'static [AtomicUsize; 3],
    polarity: bool,
    jobs: usize,
) {
    while THREADS.load(Ordering::Relaxed) >= jobs {
        core::hint::spin_loop();
    }

    THREADS.fetch_add(1, Ordering::Relaxed);
    tokio::spawn(async move {
        let mut a_engine = engine::Engine::new(a.path.as_ref(), fen);
        let mut b_engine = engine::Engine::new(b.path.as_ref(), fen);

        let (w, b) = if !polarity { (&a, &b) } else { (&b, &a) };

        let mut tc = (time, time); // w | b
        let mut overtime = 0;
        let mut r = [0.0; 2];

        'a: while game.result().is_none() {
            if !a_engine.get_move(&mut game, &mut tc, inc).await {
                overtime = 1;
                break 'a;
            }

            if game.can_declare_draw() {
                game.declare_draw();
            }

            if game.result().is_some() {
                break 'a;
            }

            if !b_engine.get_move(&mut game, &mut tc, inc).await {
                overtime = 2;
                break 'a;
            }

            if game.can_declare_draw() {
                game.declare_draw();
            }
        }

        if overtime == 1 {
            game_result[flip(2, polarity, 2)].fetch_add(1, Ordering::Relaxed);
            r[flip(1, polarity, 1)] = 1.0;
        } else if overtime == 2 {
            game_result[flip(0, polarity, 2)].fetch_add(1, Ordering::Relaxed);
            r[flip(0, polarity, 1)] = 1.0;
        } else {
            match game.result() {
                Some(chess::GameResult::WhiteCheckmates) => {
                    game_result[flip(0, polarity, 2)].fetch_add(1, Ordering::Relaxed);
                    r[flip(0, polarity, 1)] = 1.0;
                }
                Some(chess::GameResult::BlackCheckmates) => {
                    game_result[flip(2, polarity, 2)].fetch_add(1, Ordering::Relaxed);
                    r[flip(1, polarity, 1)] = 1.0;
                }
                Some(
                    chess::GameResult::DrawAccepted
                    | chess::GameResult::DrawDeclared
                    | chess::GameResult::Stalemate,
                ) => {
                    game_result[1].fetch_add(1, Ordering::Relaxed);
                    r = [0.5; 2];
                }
                _ => unreachable!(),
            }
        }

            println!("ok");
        let (mut w_pe, mut b_pe, mut w_e, mut b_e) = elo::update(&a.elo, &b.elo, r[0], r[1]);
            println!("ok");

        if polarity {
            core::mem::swap(&mut w_pe, &mut b_pe);
            core::mem::swap(&mut w_e, &mut b_e);
        }

        let filename = pgn::export_pgn(&game, &w.name, &b.name, fen, w_e, b_e);

        println!("\x1b[1;32mInfo:\x1b[0m {} \x1b[90m({w_pe:.0}→{w_e:.0})\x1b[0m vs {} \x1b[90m({b_pe:.0}→{b_e:.0})\x1b[0m was exported to {filename}", a.name, b.name);

        THREADS.fetch_sub(1, Ordering::Relaxed);
    });
}
