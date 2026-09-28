.pragma library

// The symbols a password is masked with. Each keystroke draws one at random; the symbol
// never depends on the character typed, so the field gives nothing about the password
// away except its length — the same as a row of dots would.
//
// Each run names the bundled font that draws it, since Qt's fallback between fonts is by
// script and would reach for whatever the system has instead.

const SYMBOLS = "Noto Sans Symbols"
const SYMBOLS2 = "Noto Sans Symbols 2"
const RUNIC = "Noto Sans Runic"

// Asks for the plain glyph rather than the emoji: the zodiac and some planets default to
// emoji presentation, and Qt will reach past the named family to a colour emoji font.
const TEXT_STYLE = "︎"

function range(from, to, family) {
    const out = []
    for (let c = from; c <= to; c++)
        out.push({ text: String.fromCodePoint(c) + TEXT_STYLE, family: family })
    return out
}

const pool = [].concat(
    // Alchemical symbols: the elements, metals, salts and processes.
    range(0x1F700, 0x1F773, SYMBOLS),
    // The planets, moon and sun.
    range(0x263D, 0x2647, SYMBOLS),
    [{ text: "☉" + TEXT_STYLE, family: SYMBOLS2 }],
    // The zodiac.
    range(0x2648, 0x2653, SYMBOLS),
    // The Elder and Younger Futhark, without the punctuation and numerals at the end.
    range(0x16A0, 0x16EA, RUNIC)
)

// Every symbol in the pool, run together by font: drawn once at startup so each font is
// loaded and each glyph rasterised before the first keystroke, rather than on it.
function byFamily() {
    const runs = {}
    for (const g of pool)
        runs[g.family] = (runs[g.family] || "") + g.text
    return Object.keys(runs).map(f => ({ family: f, text: runs[f] }))
}

// A symbol not among `recent`, so neighbouring slots never repeat.
function pick(recent) {
    for (let tries = 0; tries < 8; tries++) {
        const g = pool[Math.floor(Math.random() * pool.length)]
        if (!recent.some(r => r === g.text))
            return g
    }
    return pool[Math.floor(Math.random() * pool.length)]
}
