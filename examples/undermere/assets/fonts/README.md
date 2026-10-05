# DejaVu Sans Mono

`DejaVuSansMono.ttf` is the typeface Undermere draws every glyph in, compiled into the game by `src/typeface.rs`.
It is the unmodified file from the DejaVu fonts' 2.37 release, `dejavu-fonts-ttf-2.37.tar.bz2` from <https://github.com/dejavu-fonts/dejavu-fonts/releases/tag/version_2_37>, and its SHA-256 is `b4a6c3e4faab8773f4ff761d56451646409f29abedd68f05d38c2df667d3c582`.

## Why this face

The terminal otherwise draws in the system's monospace family, which is Courier on macOS, another face on Linux, and only ASCII in a browser.
DejaVu Sans Mono has every box rule, block shade, arrow and symbol the engine's panels and the game draw, so the game looks the same everywhere, and a test reads the face's own character map to keep it that way.

## The licence

`LICENSE` is the release's own licence file, shipped beside the font as it asks.
The glyphs are Bitstream Vera's, under the Bitstream Vera licence, glyphs imported from the Arev fonts are under Tavmjong Bah's licence of the same terms, and DejaVu's changes are in the public domain.
Both licences let anyone use, copy, redistribute and bundle the font with other software, free of charge, on the condition that the copyright notices and the licence travel with every copy, which is what this folder does.
They forbid selling the font by itself, and a modified copy must drop the Bitstream, Vera, Tavmjong Bah and Arev names; Undermere sells nothing, and the file here is not modified.
