# Third-party notices

ianox itself is released under the [MIT License](LICENSE). It includes or links to the
following third-party material.

## Fonts bundled in the application

The interface uses two fonts, shipped inside the application as web fonts through the
[Fontsource](https://fontsource.org) packages:

- **Inter** — Copyright 2016 The Inter Project Authors (<https://github.com/rsms/inter>)
- **JetBrains Mono** — Copyright 2020 The JetBrains Mono Project Authors (<https://github.com/JetBrains/JetBrainsMono>)

Both are licensed under the SIL Open Font License, Version 1.1, reproduced below.

```text
This Font Software is licensed under the SIL Open Font License, Version 1.1.
This license is copied below, and is also available with a FAQ at:
http://scripts.sil.org/OFL


-----------------------------------------------------------
SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007
-----------------------------------------------------------

PREAMBLE
The goals of the Open Font License (OFL) are to stimulate worldwide
development of collaborative font projects, to support the font creation
efforts of academic and linguistic communities, and to provide a free and
open framework in which fonts may be shared and improved in partnership
with others.

The OFL allows the licensed fonts to be used, studied, modified and
redistributed freely as long as they are not sold by themselves. The
fonts, including any derivative works, can be bundled, embedded,
redistributed and/or sold with any software provided that any reserved
names are not used by derivative works. The fonts and derivatives,
however, cannot be released under any other type of license. The
requirement for fonts to remain under this license does not apply
to any document created using the fonts or their derivatives.

DEFINITIONS
"Font Software" refers to the set of files released by the Copyright
Holder(s) under this license and clearly marked as such. This may
include source files, build scripts and documentation.

"Reserved Font Name" refers to any names specified as such after the
copyright statement(s).

"Original Version" refers to the collection of Font Software components as
distributed by the Copyright Holder(s).

"Modified Version" refers to any derivative made by adding to, deleting,
or substituting -- in part or in whole -- any of the components of the
Original Version, by changing formats or by porting the Font Software to a
new environment.

"Author" refers to any designer, engineer, programmer, technical
writer or other person who contributed to the Font Software.

PERMISSION & CONDITIONS
Permission is hereby granted, free of charge, to any person obtaining
a copy of the Font Software, to use, study, copy, merge, embed, modify,
redistribute, and sell modified and unmodified copies of the Font
Software, subject to the following conditions:

1) Neither the Font Software nor any of its individual components,
in Original or Modified Versions, may be sold by itself.

2) Original or Modified Versions of the Font Software may be bundled,
redistributed and/or sold with any software, provided that each copy
contains the above copyright notice and this license. These can be
included either as stand-alone text files, human-readable headers or
in the appropriate machine-readable metadata fields within text or
binary files as long as those fields can be easily viewed by the user.

3) No Modified Version of the Font Software may use the Reserved Font
Name(s) unless explicit written permission is granted by the corresponding
Copyright Holder. This restriction only applies to the primary font name as
presented to the users.

4) The name(s) of the Copyright Holder(s) or the Author(s) of the Font
Software shall not be used to promote, endorse or advertise any
Modified Version, except to acknowledge the contribution(s) of the
Copyright Holder(s) and the Author(s) or with their explicit written
permission.

5) The Font Software, modified or unmodified, in part or in whole,
must be distributed entirely under this license, and must not be
distributed under any other license. The requirement for fonts to
remain under this license does not apply to any document created
using the Font Software.

TERMINATION
This license becomes null and void if any of the above conditions are
not met.

DISCLAIMER
THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT
OF COPYRIGHT, PATENT, TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL THE
COPYRIGHT HOLDER BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
INCLUDING ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL
DAMAGES, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM
OTHER DEALINGS IN THE FONT SOFTWARE.
```

## JavaScript packages bundled in the frontend

| Package | Version | License |
| --- | --- | --- |
| `@fontsource-variable/inter` | 5.3.0 | OFL-1.1 |
| `@fontsource-variable/jetbrains-mono` | 5.3.0 | OFL-1.1 |
| `@tauri-apps/api` | 2.12.4 | Apache-2.0 OR MIT |
| `@tauri-apps/plugin-dialog` | 2.8.4 | MIT OR Apache-2.0 |
| `@xterm/xterm` | 6.0.0 | MIT |
| `@xterm/addon-fit` | 0.11.0 | MIT |
| `dompurify` | 3.4.17 | (MPL-2.0 OR Apache-2.0) |
| `marked` | 18.1.3 | MIT |
| `svelte` | 5.57.3 | MIT |

The build tools (Vite, TypeScript, the Tauri CLI and similar) are used to produce the
application and are not part of what is distributed.

## Rust crates

The backend is built from 483 crates (counting the whole dependency graph, including
platform-specific ones). Their licenses, grouped:

| License family | Crates |
| --- | --- |
| MIT and/or Apache-2.0 (alone or combined with other permissive licenses) | 450 |
| Unicode-3.0 | 18 |
| BSD, ISC, Zlib, Unlicense, CDLA-Permissive and similar permissive licenses | 10 |
| MPL-2.0 (file-level copyleft) | 5 |

None of them is licensed *only* under the GPL or AGPL (two offer the LGPL as one option among several, alongside MIT). Five crates use the
[MPL-2.0](https://www.mozilla.org/MPL/2.0/), a file-level copyleft: `cssparser`, `cssparser-macros`, `dtoa-short`, `option-ext`, `selectors`. Their source
is available from crates.io and has not been modified by ianox. Where a crate offers a
choice of licenses (for example `MIT OR Apache-2.0`), ianox uses it under the MIT option.

To regenerate the full list with each crate's license:

```sh
cargo metadata --format-version 1 --manifest-path src-tauri/Cargo.toml
```

## System libraries

On Linux ianox links dynamically against the system's GTK 3 and WebKitGTK (LGPL), which are installed by your distribution and are not redistributed by ianox.

## Data

The model catalogue (names, context windows and prices) is downloaded at run time from
[models.dev](https://models.dev) and cached on your machine. It is not redistributed with
ianox.

## Trademarks and affiliation

ianox is an independent project. It is not affiliated with, endorsed by or sponsored by
Anthropic, OpenAI, DeepSeek, Ollama, Google or any other provider or
project named in its documentation or interface. Those names are used only to say which
services it can talk to, and belong to their owners.
