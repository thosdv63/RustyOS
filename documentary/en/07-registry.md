# 07 — Registry

Every operating system needs a place to keep settings — the desktop color, the
active user, whether the first-run setup has finished. Rusty OS keeps all of this
in a registry, borrowing the name and the hierarchical-key idea from Windows but
keeping the implementation deliberately small and readable. It lives under
`kernel/rgst`.

## Format and structure

The registry is a plain-text, key-value store. Each entry is a path and a typed
value, written one per line in a form like `Sistem/Masaustu/Renk=u32:5249032` —
the path, an equals sign, the type, a colon, and the value. Three types are
supported: unsigned 32-bit integers, strings, and booleans. Keeping it human-
readable was a deliberate choice; the entire system configuration can be inspected
by simply reading the file, and the same text format is what the registry editor
application shows and edits.

In memory the registry is a list of entries behind a lock, with the usual
operations — get a value by path, set a value, list all keys with a given prefix.
On top of these sit small typed accessors, so the rest of the kernel can ask for
a `u32` at a path with a default, without unpacking the type by hand. There is also
a set of sensible defaults the system falls back to when no registry exists yet:
the system name and version, the language, the desktop and taskbar colors, the
timezone, and an initial user.

## Persistence

The whole store fits in four kilobytes and is kept in a single file,
`RSYS/REGISTRY.DAT`, on the system disk. Loading and saving live in `kernel/rgst/disk`,
and they work by walking the file's cluster chain directly rather than going
through the higher-level file API, reading or writing the raw sectors that back the
registry file.

There's a hard-won detail buried here. Saving the registry needs the first cluster
of the registry file, and looking that up every time means rescanning the directory.
To avoid that, the first cluster is cached the moment the registry is loaded. On a
freshly installed disk this cache starts empty, and an early version would crash
when the first save happened before anything had populated it — fixed by looking
the cluster up on demand if the cache is cold. It's a small thing, but it's exactly
the sort of bug that only appears on a real installed system and never in a quick
test.

## The drive table and caches

The registry module also owns the table of mounted drives — each with its letter,
label, kind, size, and filesystem — and a few atomic caches for values the userland
reads constantly, like the desktop and taskbar colors and the current user's
permission level. Caching these avoids taking the registry lock on every single
frame the desktop draws.

## Recovery

The final piece is a recovery mode, in `kernel/rgst/recovery`. On boot the kernel
can check whether the essential files are present — the registry itself, the
userland's `CORE.BIN`, and the expected user folders — and if something critical is
missing, it enters a rescue mode that repairs the damage. It can recreate the
directory structure, rewrite a lost registry from the in-memory defaults, and
restore `CORE.BIN` from a copy embedded directly in the kernel image. This is what
lets a partially damaged installation heal itself and boot again, rather than
simply failing.