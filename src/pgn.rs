use std::str::FromStr;
use std::time::*;

pub fn export_pgn(game: &chess::Game, w: &str, b: &str, fen: &str, elo: Option<(f32, f32)>) -> String {
    use std::fmt::Write as _;

    let mut pgn = String::new();
    writeln!(pgn, r#"[Event "AB test"]"#).unwrap();
    writeln!(pgn, r#"[Site "https://github.com/funnsam/uci-ab-test"]"#).unwrap();
    writeln!(pgn, r#"[Date "??"]"#).unwrap();
    writeln!(pgn, r#"[Round "??"]"#).unwrap();
    writeln!(pgn, r#"[White "{w}"]"#).unwrap();
    writeln!(pgn, r#"[Black "{b}"]"#).unwrap();
    if let Some((w_elo, b_elo)) = elo {
        writeln!(pgn, r#"[WhiteElo "{w_elo:.0}"]"#).unwrap();
        writeln!(pgn, r#"[BlackElo "{b_elo:.0}"]"#).unwrap();
    }

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
        if pgn.lines().last().unwrap().len() + ((i + 1).ilog10() + 2) as usize >= 100 {
            pgn = pgn.trim_end().to_string();
            writeln!(pgn).unwrap();
        }

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

    let filename = format!("game_{}.pgn", UNIX_EPOCH.elapsed().unwrap().as_millis());

    std::fs::write(&filename, pgn).unwrap();

    filename
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
            if board.piece_on(m.get_source()).unwrap() == piece {
                pieces |= chess::BitBoard::from_square(m.get_source());
            }
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
    }

    let next = board.make_move_new(m);

    // en passant
    let captured = board.combined().popcnt() != next.combined().popcnt();

    if captured {
        if piece == chess::Piece::Pawn {
            san.push((b'a' + m.get_source().get_file() as u8) as char);
        }

        san += "x";
    }

    san += &m.get_dest().to_string();

    if let Some(p) = m.get_promotion() {
        san += "=";
        san += &p.to_string(chess::Color::White);
    }

    if matches!(next.status(), chess::BoardStatus::Checkmate) {
        san += "#";
    } else if next.checkers().0 != 0 {
        san += "+";
    }

    *board = next;

    san
}
