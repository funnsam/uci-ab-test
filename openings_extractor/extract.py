import chess
import chess.engine
import chess.pgn
import itertools

db = open("./db.pgn")
fen = ""

engine = chess.engine.SimpleEngine.popen_uci("stockfish")

def analyse(game, time_limit = 0.01):
    result = engine.analyse(board, chess.engine.Limit(time=time_limit))
    return result['score']

print()

i = 0
while True:
    game = chess.pgn.read_game(db)

    if not game:
        break

    board = game.board()

    for m in itertools.islice(game.mainline_moves(), 8):
        board.push(m)

    score = analyse(board)

    if abs(score.relative.score(mate_score = 10000)) < 100 and board.fen() not in fen:
        fen += board.fen()
        fen += "\n"

        i += 1

        print("\x1b[1A{}".format(i))

        if i >= 256:
            break

db.close()
engine.close()

openings = open("../openings.txt", "w")
openings.write(fen)
openings.close()
