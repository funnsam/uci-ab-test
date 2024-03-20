use std::io::{self, BufRead as _, Write as _};
use std::process::*;
use std::time::*;

pub struct Engine<'a> {
    exec: Child,
    fen: &'a str,
}

impl<'a> Drop for Engine<'a> {
    fn drop(&mut self) {
        _ = self.exec.kill();
    }
}

impl<'a> Engine<'a> {
    pub fn get_name(exec: &str) -> Option<String> {
        let mut exec = std::process::Command::new(exec)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        writeln!(exec.stdin.as_ref().unwrap(), "uci").unwrap();

        let mut lines = io::BufReader::new(exec.stdout.as_mut().unwrap()).lines();
        while let Some(Ok(l)) = lines.next() {
            let mut tokens = l.split_whitespace();
            let cmd = tokens.next();

            if matches!(cmd, Some("id")) {
                if matches!(tokens.next(), Some("name")) {
                    return Some(l.splitn(3, ' ').nth(2).unwrap().to_string());
                }
            } else if matches!(cmd, Some("uciok")) {
                break;
            }
        }

        None
    }

    pub fn new(exec: &str, fen: &'a str) -> Self {
        let mut exec = std::process::Command::new(exec)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        writeln!(exec.stdin.as_ref().unwrap(), "uci").unwrap();
        writeln!(exec.stdin.as_ref().unwrap(), "isready").unwrap();
        writeln!(exec.stdin.as_ref().unwrap(), "ucinewgame").unwrap();

        for l in io::BufReader::new(exec.stdout.as_mut().unwrap()).lines() {
            if l.map_or(false, |a| a.starts_with("readyok")) {
                break;
            }
        }

        Self { exec, fen }
    }

    pub fn get_move(
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

        self.find_best_in_time().map_or(false, |m| {
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

    fn find_best_in_time(&mut self) -> Option<chess::ChessMove> {
        let mut lines = io::BufReader::new(self.exec.stdout.as_mut().unwrap()).lines();
        while let Some(Ok(l)) = lines.next() {
            let mut tokens = l.split_whitespace();
            if matches!(tokens.next(), Some("bestmove")) {
                return Some(move_from_uci(&tokens.next().unwrap()));
            }
        }

        None
    }

    pub fn send_features(&mut self, features: &crate::tune::FeatureVector<i32>) {
        write!(self.exec.stdin.as_mut().unwrap(), "setoption name FeatureVector ");
        for i in features.iter() {
            write!(self.exec.stdin.as_mut().unwrap(), "{i}");
        }
        writeln!(self.exec.stdin.as_mut().unwrap());
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
