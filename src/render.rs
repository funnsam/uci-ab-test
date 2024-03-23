use chess::*;

const PIECES: [char; 6] = ['󰡙', '󰡘', '󰡜', '󰡛', '󰡚', '󰡗'];

pub fn render(board: &Board, w_name: &str, b_name: &str, last_move: Option<ChessMove>) {
    println!("{b_name}");
    for rank in ALL_RANKS.into_iter().rev() {
        for file in ALL_FILES {
            let square = Square::make_square(rank, file);
            let piece = board.piece_on(square);
            let color = board.color_on(square);
            let parity = (rank.to_index() + file.to_index()) & 1 == 1;
            let highlight = last_move.map_or(false, |m| square == m.get_source() || square == m.get_dest());

            print!("\x1b[{}{}", if matches!(piece, Some(Piece::King)) && color == Some(board.side_to_move()) && board.checkers().0 != 0 {
                "48;2;235;94;78"
            } else if parity && highlight {
                "48;2;137;140;71"
            } else if highlight {
                "48;2;113;108;39"
            } else if parity {
                "48;2;160;145;121"
            } else {
                "48;2;121;91;66"
            }, match color {
                Some(Color::White) => ";97m",
                Some(Color::Black) => ";30m",
                None => "m",
            });

            print!("{} ", piece.map_or(' ', |a| PIECES[a.to_index()]));
        }

        println!("\x1b[0m");
    }

    println!("{b_name}\x1b[10A");
}
