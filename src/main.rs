use clap::*;
use std::str::FromStr;
use std::sync::{atomic::*, *};

mod elo;
mod engine;
mod pgn;
mod tune;
mod render;

#[derive(Debug, Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Play(PlayArgs),
    Tune(TuneArgs),
    Watch(WatchArgs),
}

#[derive(Debug, Args)]
struct PlayArgs {
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

#[derive(Debug, Args)]
struct TuneArgs {
    engine: String,

    initial_theta: String,

    #[arg(long, default_value = "openings.txt")]
    opening_positions: String,

    #[arg(short, long, default_value_t = 100)]
    iterations: usize,

    #[arg(short = 'n', long, default_value_t = 2)]
    play_positions: usize,

    #[arg(long, default_value_t = 1)]
    seed: i32,
    #[arg(short, long, default_value_t = 1)]
    jobs: usize,
}

#[derive(Debug, Args)]
struct WatchArgs {
    w: String,
    b: String,

    time: usize,
    inc: usize,

    fen: String,
}

static THREADS: AtomicUsize = AtomicUsize::new(0);

fn main() {
    let args = Args::parse();

    match args.command {
        Command::Play(play_args) => play(play_args),
        Command::Tune(tune_args) => tune(tune_args),
        Command::Watch(watch_args) => watch(watch_args),
    }
}

fn play(args: PlayArgs) {
    let game_result = Arc::new([
        AtomicUsize::new(0),
        AtomicUsize::new(0),
        AtomicUsize::new(0),
    ]); // a win | draw | b win

    let fens = get_fens(&args.opening_positions, args.play_positions);

    let a_player = Arc::new(Player {
        path: args.a.as_str().into(),
        name: engine::Engine::get_name(args.a.as_str())
            .map_or_else(|| args.a.as_str().into(), |a| a.as_str().into()),
    });
    let b_player = Arc::new(Player {
        path: args.b.as_str().into(),
        name: engine::Engine::get_name(args.b.as_str())
            .map_or_else(|| args.b.as_str().into(), |a| a.as_str().into()),
    });
    let elos = Arc::new(Mutex::new((args.a_elo, args.b_elo)));

    println!("\x1b[1;32mInfo:\x1b[0m initialization complete");

    for fen in fens.iter() {
        let game = chess::Game::from_str(fen).unwrap();
        let fen = fen.as_str().into();

        play_single(
            Arc::clone(&a_player),
            Arc::clone(&b_player),
            Some(Arc::clone(&elos)),
            game.clone(),
            Arc::clone(&fen),
            args.time,
            args.inc,
            Arc::clone(&game_result),
            false,
            args.jobs,
        );

        if !args.biased {
            play_single(
                Arc::clone(&b_player),
                Arc::clone(&a_player),
                Some(Arc::clone(&elos)),
                game.clone(),
                Arc::clone(&fen),
                args.time,
                args.inc,
                Arc::clone(&game_result),
                true,
                args.jobs,
            );
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

    let elos = elos.lock().unwrap();

    println!("\n\n\x1b[1m{} SUMMARY {}\x1b[0m", l_indent, r_indent);
    println!("\x1b[1mTotal:\x1b[0m {total} games");
    println!("  \x1b[32m{a}\x1b[90m\x1b[{d_pad}C{d}\x1b[31m\x1b[{b_pad}C{b}\x1b[0m");
    println!("  \x1b[32m{a_bar}\x1b[90m{d_bar}\x1b[31m{b_bar}\x1b[0m");
    println!("\n \x1b[1mElo:\x1b[0m A: {:.0}, B: {:.0}", elos.0, elos.1);
}

fn tune(args: TuneArgs) {
    let theta = tune::FeatureVector::<f32>::from_binary(&std::fs::read(args.initial_theta).unwrap());
    let fens = get_fens(&args.opening_positions, args.play_positions);

    tune::tune(args.iterations, &args.engine, theta, &fens, args.seed, args.jobs);
}

fn watch(args: WatchArgs) {
    let w_name = engine::Engine::get_name(args.w.as_str()).unwrap_or_else(|| args.w.clone());
    let b_name = engine::Engine::get_name(args.b.as_str()).unwrap_or_else(|| args.b.clone());

    let mut w_engine = engine::Engine::new(&args.w, &args.fen);
    let mut b_engine = engine::Engine::new(&args.b, &args.fen);

    let mut game = chess::Game::from_str(&args.fen).unwrap();
    let mut game_result = [0; 3];

    let mut tc = (args.time, args.time); // w | b
    let mut overtime = 0;

    render::render(&game.current_position(), &w_name, &b_name, None);

    'a: while game.result().is_none() {
        if let Some(m) = w_engine.get_move(&mut game, &mut tc, args.inc) {
            render::render(&game.current_position(), &w_name, &b_name, Some(m));
        } else {
            overtime = 1;
            break 'a;
        }


        if game.can_declare_draw() {
            game.declare_draw();
        }

        if game.result().is_some() {
            break 'a;
        }

        if let Some(m) = b_engine.get_move(&mut game, &mut tc, args.inc) {
            render::render(&game.current_position(), &w_name, &b_name, Some(m));
        } else {
            overtime = 2;
            break 'a;
        }

        if game.can_declare_draw() {
            game.declare_draw();
        }
    }

    if overtime == 1 {
        game_result[2] += 1;
    } else if overtime == 2 {
        game_result[0] += 1;
    } else {
        match game.result() {
            Some(chess::GameResult::WhiteCheckmates) => {
                game_result[0] += 1;
            }
            Some(chess::GameResult::BlackCheckmates) => {
                game_result[2] += 1;
            }
            Some(
                chess::GameResult::DrawAccepted
                | chess::GameResult::DrawDeclared
                | chess::GameResult::Stalemate,
                ) => {
                game_result[1] += 1;
            }
            _ => unreachable!(),
        }
    }

    let filename = pgn::export_pgn(&game, &w_name, &b_name, &args.fen, None);

    println!("\x1b[10B\x1b[1;32mInfo:\x1b[0m {w_name} vs {b_name} was exported to {filename}");
}

fn get_fens(file: &str, n: usize) -> Vec<String> {
    std::fs::read_to_string(file)
        .unwrap()
        .trim()
        .lines()
        .take(n)
        .map(|a| a.to_string())
        .collect()
        // .map(|fen| (chess::Game::from_str(fen).unwrap(), fen.to_string()))
        // .collect::<Vec<(chess::Game, String)>>();
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
}

fn play_single(
    a: Arc<Player>,
    b: Arc<Player>,
    elos: Option<Arc<Mutex<(f32, f32)>>>,
    game: chess::Game,
    fen: Arc<str>,
    time: usize,
    inc: usize,
    game_result: Arc<[AtomicUsize; 3]>,
    polarity: bool,
    jobs: usize,
) {
    let a_engine = engine::Engine::new(a.path.as_ref(), &fen);
    let b_engine = engine::Engine::new(b.path.as_ref(), &fen);

    play_with_engine(a_engine, b_engine, Arc::clone(&a.name), Arc::clone(&b.name), elos, game, fen, time, inc, game_result, polarity, jobs)
}

fn play_with_engine(
    mut a_engine: engine::Engine,
    mut b_engine: engine::Engine,

    a_name: Arc<str>,
    b_name: Arc<str>,

    elos: Option<Arc<Mutex<(f32, f32)>>>,

    mut game: chess::Game,
    fen: Arc<str>,

    time: usize,
    inc: usize,

    game_result: Arc<[AtomicUsize; 3]>,
    polarity: bool,
    jobs: usize,
) {
    while THREADS.load(Ordering::Relaxed) >= jobs {
        core::hint::spin_loop();
    }

    THREADS.fetch_add(1, Ordering::Relaxed);
    std::thread::spawn(move || {
        let (w_name, b_name) = if !polarity { (Arc::clone(&a_name), Arc::clone(&b_name)) } else { (Arc::clone(&b_name), Arc::clone(&a_name)) };

        let mut tc = (time, time); // w | b
        let mut overtime = 0;
        let mut r = [0.0; 2];

        'a: while game.result().is_none() {
            if a_engine.get_move(&mut game, &mut tc, inc).is_none() {
                overtime = 1;
                break 'a;
            }

            if game.can_declare_draw() {
                game.declare_draw();
            }

            if game.result().is_some() {
                break 'a;
            }

            if b_engine.get_move(&mut game, &mut tc, inc).is_none() {
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

        if let Some(elos) = elos {
            let (mut w_pe, mut b_pe, mut w_e, mut b_e) = elo::update(&elos, r[0], r[1]);

            if polarity {
                core::mem::swap(&mut w_pe, &mut b_pe);
                core::mem::swap(&mut w_e, &mut b_e);
            }

            let filename = pgn::export_pgn(&game, &w_name, &b_name, &fen, Some((w_e, b_e)));

            println!("\x1b[1;32mInfo:\x1b[0m {w_name} \x1b[90m({w_pe:.0}→{w_e:.0})\x1b[0m vs {b_name} \x1b[90m({b_pe:.0}→{b_e:.0})\x1b[0m was exported to {filename}");
        } else {
            let filename = pgn::export_pgn(&game, &w_name, &b_name, &fen, None);

            println!("\x1b[1;32mInfo:\x1b[0m {w_name} vs {b_name} was exported to {filename}");
        }

        THREADS.fetch_sub(1, Ordering::Relaxed);
    });
}
