# @kte/xterm-addon-khmer (planned)

xterm.js addon that registers a Unicode provider implementing the
[KTE Width Spec](../../spec/kte-width-spec.md), backed by KTE compiled to WebAssembly.

**Status: not started (Phase 3).** The npm package scope and name are placeholders.

Planned usage:

```ts
import { Terminal } from '@xterm/xterm';
import { KhmerAddon } from '@kte/xterm-addon-khmer';

const term = new Terminal({ allowProposedApi: true });
term.loadAddon(new KhmerAddon());
term.unicode.activeVersion = 'kte';
```

Rendering with the WebGL renderer needs cluster-level rasterization upstream in xterm.js;
see [docs/upstream](../../docs/upstream/README.md).
