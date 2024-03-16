use clap::*;
use std::io::{self, BufRead as _, Write as _};
use std::process::*;
use std::str::FromStr;
use std::sync::atomic::*;
use std::time::*;

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

    for (game, fen) in fens.iter() {
        let a = unsafe { core::mem::transmute::<_, &'static _>(args.a.as_str()) };
        let b = unsafe { core::mem::transmute::<_, &'static _>(args.b.as_str()) };
        let fen = unsafe { core::mem::transmute::<_, &'static _>(fen.as_str()) };
        let game_result =
            unsafe { core::mem::transmute::<_, &'static [AtomicUsize; 3]>(&game_result) };

        play(
            a,
            b,
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
                b,
                a,
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

    let indents_length = term_size.0 - " SUMMARY ".len();
    let l_indent_length = indents_length / 2;
    let r_indent_length = indents_length - l_indent_length;

    let l_indent = "=".repeat(l_indent_length);
    let r_indent = "=".repeat(r_indent_length);

    println!("\n\n\x1b[1m{} SUMMARY {}\x1b[0m", l_indent, r_indent);
    println!("\x1b[1mTotal:\x1b[0m {total} games");
    println!("   \x1b[32mA wins: {a}\x1b[90m | Draws: {d} | \x1b[31mA loses: {b}\x1b[0m");
    println!("  \x1b[32m{a_bar}\x1b[90m{d_bar}\x1b[31m{b_bar}\x1b[0m");
}

fn flip(idx: usize, flip: bool) -> usize {
    if flip {
        2 - idx
    } else {
        idx
    }
}

async fn play(
    a: &'static str,
    b: &'static str,
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
        let mut a_engine = Engine::new(a, fen);
        let mut b_engine = Engine::new(b, fen);

        let mut tc = (time, time); // w | b

        let mut overtime = 0;

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
            game_result[flip(2, polarity)].fetch_add(1, Ordering::Relaxed);
        } else if overtime == 2 {
            game_result[flip(0, polarity)].fetch_add(1, Ordering::Relaxed);
        } else {
            match game.result() {
                Some(chess::GameResult::WhiteCheckmates) => {
                    game_result[flip(0, polarity)].fetch_add(1, Ordering::Relaxed);
                }
                Some(chess::GameResult::BlackCheckmates) => {
                    game_result[flip(2, polarity)].fetch_add(1, Ordering::Relaxed);
                }
                Some(
                    chess::GameResult::DrawAccepted
                    | chess::GameResult::DrawDeclared
                    | chess::GameResult::Stalemate,
                ) => {
                    game_result[1].fetch_add(1, Ordering::Relaxed);
                }
                _ => unreachable!(),
            }
        }

        export_pgn(
            &game,
            if !polarity { a } else { b },
            if polarity { a } else { b },
            fen,
        );

        println!("\x1b[1;32mInfo:\x1b[0m a game was ended");

        THREADS.fetch_sub(1, Ordering::Relaxed);
    });
}

struct Engine<'a> {
    exec: Child,
    fen: &'a str,
}

impl<'a> Drop for Engine<'a> {
    fn drop(&mut self) {
        _ = self.exec.kill();
    }
}

impl<'a> Engine<'a> {
    fn new(exec: &str, fen: &'a str) -> Self {
        let mut exec = std::process::Command::new(exec)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        writeln!(exec.stdin.as_ref().unwrap(), "uci").unwrap();
        writeln!(exec.stdin.as_ref().unwrap(), "isready").unwrap();
        writeln!(exec.stdin.as_ref().unwrap(), "ucinewgame").unwrap();

        wait_readyok(exec.stdout.as_mut().unwrap());

        Self { exec, fen }
    }

    async fn get_move(
        &mut self,
        game: &mut chess::Game,
        tc: &mut (usize, usize),
        inc: usize,
    ) -> bool {
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
            game.actions()
                .iter()
                .map(|a| match a {
                    chess::Action::MakeMove(m) => m.to_string(),
                    _ => "".to_string(),
                })
                .collect::<Vec<String>>()
                .join(" ")
        )
        .unwrap();

        let start = Instant::now();

        writeln!(
            self.exec.stdin.as_ref().unwrap(),
            "go wtime {} winc {inc} btime {} binc {inc}",
            tc0,
            tc1
        )
        .unwrap();

        let m = tokio::time::timeout(Duration::from_millis(*mt as u64), async move {
            let mut lines = io::BufReader::new(self.exec.stdout.as_mut().unwrap()).lines();
            while let Some(Ok(l)) = lines.next() {
                let mut tokens = l.split_whitespace();
                if matches!(tokens.next(), Some("bestmove")) {
                    return Some(move_from_uci(&tokens.next().unwrap()));
                }
            }

            None
        })
        .await;

        m.ok().flatten().map_or(false, |m| {
            let used_time = start.elapsed().as_millis() as usize;

            if !game.current_position().legal(m) {
                return false;
            }

            game.make_move(m);

            let time = (*mt + inc).checked_sub(used_time);
            if let Some(time) = time {
                *mt = time;
                true
            } else {
                false
            }
        })
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

fn export_pgn(game: &chess::Game, w: &str, b: &str, fen: &str) {
    use std::fmt::Write as _;

    let mut pgn = String::new();
    writeln!(pgn, r#"[Event "AB test"]"#).unwrap();
    writeln!(pgn, r#"[Site "https://github.com/funnsam/uci-ab-test"]"#).unwrap();
    writeln!(pgn, r#"[Date "??"]"#).unwrap();
    writeln!(pgn, r#"[Round "??"]"#).unwrap();
    writeln!(pgn, r#"[White "{w}"]"#).unwrap();
    writeln!(pgn, r#"[Black "{b}"]"#).unwrap();

    let result = match game.result() {
        Some(chess::GameResult::BlackResigns | chess::GameResult::WhiteCheckmates) => "1-0",
        Some(chess::GameResult::WhiteResigns | chess::GameResult::BlackCheckmates) => "0-1",
        Some(_) => "1/2-1/2",
        None => "*",
    };

    writeln!(pgn, r#"[Result "{result}"]"#).unwrap();
    writeln!(pgn, r#"[FEN "{fen}"]"#).unwrap();
    writeln!(pgn).unwrap();

    let mut board = chess::Board::from_str(fen).unwrap();

    for (i, m) in game
        .actions()
        .iter()
        .filter_map(|a| match a {
            chess::Action::MakeMove(m) => Some(m),
            _ => None,
        })
        .collect::<Vec<&chess::ChessMove>>()
        .chunks(2)
        .enumerate()
    {
        write!(pgn, "{}. ", i + 1).unwrap();

        for m in m {
            let m = make_san(&mut board, **m);

            if pgn.lines().last().unwrap().len() + m.len() >= 100 {
                pgn = pgn.trim_end().to_string();
                writeln!(pgn).unwrap();
            }

            write!(pgn, "{m} ").unwrap();
        }
    }

    pgn += result;

    std::fs::write(
        format!("game_{}.pgn", UNIX_EPOCH.elapsed().unwrap().as_millis()),
        pgn,
    )
    .unwrap();
}

fn make_san(board: &mut chess::Board, m: chess::ChessMove) -> String {
    let cr = board.my_castle_rights();
    if m.get_source() == board.king_square(board.side_to_move())
        && !matches!(m.get_dest().get_file(), chess::File::D | chess::File::G)
        && matches!(
            m.get_dest().get_rank(),
            chess::Rank::First | chess::Rank::Eighth
        )
    {
        if m.get_dest().get_file() < m.get_source().get_file() && cr.has_queenside() {
            *board = board.make_move_new(m);

            return "O-O-O".to_string();
        } else if cr.has_kingside() {
            *board = board.make_move_new(m);

            return "O-O".to_string();
        }
    }

    let mut san = String::new();

    let piece = board.piece_on(m.get_source()).unwrap();
    if piece != chess::Piece::Pawn {
        san += &piece.to_string(chess::Color::White);
    };

    if piece != chess::Piece::Pawn {
        let mask = chess::BitBoard::from_square(m.get_dest());
        let mut pieces = chess::BitBoard::new(0);

        let mut g = chess::MoveGen::new_legal(board);
        g.set_iterator_mask(mask);
        for m in g {
            pieces |= chess::BitBoard::from_square(m.get_source());
        }

        pieces &= !(chess::BitBoard::from_square(m.get_source()));

        if pieces.0 != 0 {
            if (pieces & chess::get_file(m.get_source().get_file())).0 == 0 {
                san.push((b'a' + m.get_source().get_file() as u8) as char);
            } else if (pieces & chess::get_rank(m.get_source().get_rank())).0 == 0 {
                san.push((b'1' + m.get_source().get_rank() as u8) as char);
            } else {
                san += &m.get_source().to_string();
            }
        }
    } else if board.piece_on(m.get_dest()).is_some() {
        san.push((b'a' + m.get_source().get_file() as u8) as char)
    }

    if board.piece_on(m.get_dest()).is_some() {
        san += "x";
    }

    san += &m.get_dest().to_string();

    if let Some(p) = m.get_promotion() {
        san += "=";
        san += &p.to_string(chess::Color::White);
    }

    *board = board.make_move_new(m);

    if matches!(board.status(), chess::BoardStatus::Checkmate) {
        san += "#";
    } else if board.checkers().0 != 0 {
        san += "+";
    }

    san
}
