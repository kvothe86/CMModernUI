# Licences of the pictures cmui uses (checked 2026-10-27)

## football-logos.cc (club and competition logos)

Source: https://football-logos.cc/license/ (linked as "Legal" in the site footer; footer of
https://football-logos.cc also says "All logos are sourced, normalized, restored, and
vectorized by Leo4815162342" and "(c) 2026 Created by Leo4815162342").

- Logos are shown "strictly for informational, editorial, and fan-based purposes", which
  "includes research, non-commercial design work, and fan projects."
- You may not use them for commercial products or merchandise, branding or promotional
  materials, or to imply affiliation or endorsement.
- They "are the intellectual property of their respective clubs, leagues, or associations";
  "All trademarks and logos belong to their respective owners."
- Redistribution / bundling in software and attribution are not addressed at all.

Verdict: NO permission to redistribute. The package therefore contains no logo pictures and
no `cmui_crests.bin`; the player runs `install.py --crests`, which downloads the pictures for
personal use. README.txt says so and tells players not to share the generated file.

## flagcdn.com (flags)

- https://flagcdn.com only says "we appreciate backlink to https://flagpedia.net" (a request,
  not a condition) and that the flags are "based on vector files from Wikipedia Commons".
  Its Terms of Use page could not be read.
- https://flagpedia.net/about: "Flag images are in the public domain (exempt from
  copyright)." and "They are completely free for non-commercial and even commercial use."
  Redistribution is not explicitly addressed.

Verdict: probably fine to redistribute (public domain), but the terms page was not
verified, so the package also leaves flags out; `--crests` fetches them. Credited in README.

Later the same day: https://flagpedia.net/download (read by the main session) states the set is
"completely free for commercial and non-commercial use (public domain)" and asks for a backlink.
So the flags could be bundled; they are still fetched (now the 640px bitmaps, `flagcdn.com/w640`)
because the crest file is built per game anyway.

## cmui itself and mike_appguy's design

Not a licence question for the pictures, but note: the look is mike_appguy's work (forum
topic 11030, champman0102.co.uk); the author's agreement to a public release was not checked
here and is for the cmui owner to confirm. cmui contains no file from his mod.
