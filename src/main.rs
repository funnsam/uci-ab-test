use clap::*;
use std::sync::atomic::*;
use std::str::FromStr;
use std::process::*;
use std::io::{self, BufRead as _, Write as _};

#[derive(Debug, Parser)]
struct Args {
    a: String,
    b: String,

    time: usize,
    inc: usize,

    #[arg(long, default_value = "openings.txt")]
    opening_positions: String,
}

const BAR_LENGTH: usize = 20;
static THREADS: AtomicUsize = AtomicUsize::new(0);

fn main() {
    let args = Args::parse();
    let game_result = [AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0)]; // a win | draw | b win

    let fens = std::fs::read_to_string(args.opening_positions)
        .unwrap()
        .trim()
        .lines()
        .map(|fen| (chess::Game::from_str(fen).unwrap(), fen.to_string()))
        .collect::<Vec<(chess::Game, String)>>();

    for (game, fen) in fens.iter() {
        let a = unsafe { core::mem::transmute::<_, &'static _>(args.a.as_str()) };
        let b = unsafe { core::mem::transmute::<_, &'static _>(args.b.as_str()) };
        let fen = unsafe { core::mem::transmute::<_, &'static _>(fen.as_str()) };
        let game_result = unsafe { core::mem::transmute::<_, &'static [AtomicUsize; 3]>(&game_result) };

        play(a, b, game.clone(), fen, args.time, args.inc, game_result, false);
        play(b, a, game.clone(), fen, args.time, args.inc, game_result, true);
    }

    while THREADS.load(Ordering::Relaxed) != 0 {
        core::hint::spin_loop();
    }

    let total = fens.len() * 2;

    let a = game_result[0].load(Ordering::Relaxed);
    let d = game_result[1].load(Ordering::Relaxed);
    let b = game_result[2].load(Ordering::Relaxed);

    let a_bar_length = ((a as f32 / total as f32) * BAR_LENGTH as f32) as usize;
    let d_bar_length = ((d as f32 / total as f32) * BAR_LENGTH as f32) as usize;
    let b_bar_length = ((b as f32 / total as f32) * BAR_LENGTH as f32) as usize;

    let a_bar = "=".repeat(a_bar_length);
    let d_bar = "=".repeat(d_bar_length);
    let b_bar = "=".repeat(b_bar_length);

    println!("\x1b[1mTotal:\x1b[0m {total} games");
    println!("\x1b[32mA ({}) {}\x1b[90m{}\x1b[31m{} B ({})\x1b[0m", a, a_bar, d_bar, b_bar, b);
}

fn flip(idx: usize, flip: bool) -> usize {
    if flip {
        2 - idx
    } else {
        idx
    }
}

fn play(a: &'static str, b: &'static str, mut game: chess::Game, fen: &'static str, time: usize, inc: usize, game_result: &'static [AtomicUsize; 3], polarity: bool) {
    THREADS.fetch_add(1, Ordering::Relaxed);
    std::thread::spawn(move || {
        let mut a_engine = Engine::new(a, fen);
        let mut b_engine = Engine::new(b, fen);

        let mut tc = (time, time); // w | b

        let mut overtime = 0;

        'a: while game.result().is_none() {
            if !a_engine.get_move(&mut game, &mut tc, inc) {
                overtime = 1;
                break 'a;
            }

            if !b_engine.get_move(&mut game, &mut tc, inc) {
                overtime = 2;
                break 'a;
            }
        }

        if overtime == 1 {
            game_result[flip(0, polarity)].fetch_add(1, Ordering::Relaxed);
        } else if overtime == 2 {
            game_result[flip(2, polarity)].fetch_add(1, Ordering::Relaxed);
        } else {
            match game.result() {
                Some(chess::GameResult::WhiteCheckmates) => { game_result[flip(0, polarity)].fetch_add(1, Ordering::Relaxed); },
                Some(chess::GameResult::BlackCheckmates) => { game_result[flip(2, polarity)].fetch_add(1, Ordering::Relaxed); },
                Some(chess::GameResult::Stalemate) => { game_result[1].fetch_add(1, Ordering::Relaxed); },
                _ => unreachable!(),
            }
        }

        println!("ok");

        THREADS.fetch_sub(1, Ordering::Relaxed);
    });
}

struct Engine<'a> {
    exec: Child,
    fen: &'a str,
}

impl<'a> Engine<'a> {
    fn new(exec: &str, fen: &'a str) -> Self {
        let mut exec = std::process::Command::new(exec)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        writeln!(exec.stdin.as_ref().unwrap(), "uci").unwrap();
        writeln!(exec.stdin.as_ref().unwrap(), "isready").unwrap();
        writeln!(exec.stdin.as_ref().unwrap(), "ucinewgame").unwrap();

        wait_readyok(exec.stdout.as_mut().unwrap());

        Self {
            exec,
            fen,
        }
    }

    fn get_move(&mut self, game: &mut chess::Game, tc: &mut (usize, usize), inc: usize) -> bool {
        use std::time::*;

        let tc0 = tc.0;
        let tc1 = tc.1;

        let mt = match game.side_to_move() {
            chess::Color::White => &mut tc.0,
            chess::Color::Black => &mut tc.1,
        };

        writeln!(
            self.exec.stdin.as_ref().unwrap(),
            "position fen {} moves {}",
            self.fen,
            game.actions().iter()
                .map(|a| match a { chess::Action::MakeMove(m) => m.to_string(), _ => "".to_string() })
                .collect::<Vec<String>>()
                .join(" ")
        ).unwrap();

        let start = Instant::now();

        writeln!(
            self.exec.stdin.as_ref().unwrap(),
            "go wtime {} winc {inc} btime {} binc {inc}",
            tc0,
            tc1
        ).unwrap();

        let mut m = chess::ChessMove::default();
        for l in io::BufReader::new(self.exec.stdout.as_mut().unwrap()).lines() {
            if let Ok(l) = l {
                let mut tokens = l.split_whitespace();
                if matches!(tokens.next(), Some("bestmove")) {
                    m = move_from_uci(&tokens.next().unwrap());
                    break;
                }
            }
        }

        let used_time = start.elapsed().as_millis() as usize;

        game.make_move(m);

        let time = (*mt + inc).checked_sub(used_time);
        if let Some(time) = time {
            *mt = time;
            true
        } else {
            false
        }
    }
}

fn wait_readyok(out: &mut ChildStdout) {
    for l in io::BufReader::new(out).lines() {
        if l.map_or(false, |a| a.starts_with("readyok")) {
            return;
        }
    }
}

fn move_from_uci(m: &str) -> chess::ChessMove {
    let src = &m[0..2];
    let src = unsafe {
        chess::Square::new(((src.as_bytes()[1] - b'1') << 3) + (src.as_bytes()[0] - b'a'))
    };

    let dst = &m[2..4];
    let dst = unsafe {
        chess::Square::new(((dst.as_bytes()[1] - b'1') << 3) + (dst.as_bytes()[0] - b'a'))
    };

    let piece = m.as_bytes().get(4).and_then(|p| match p {
        b'n' => Some(chess::Piece::Knight),
        b'b' => Some(chess::Piece::Bishop),
        b'q' => Some(chess::Piece::Queen),
        b'r' => Some(chess::Piece::Rook),
        _ => None,
    });

    chess::ChessMove::new(src, dst, piece)
}
