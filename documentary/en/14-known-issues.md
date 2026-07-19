# 14 — Known Issues

Every honest account of a system includes the parts that don't work yet, and Rusty
OS is no exception. This chapter is a deliberately frank list of the rough edges,
the deferred work, and the assumptions that hold only because nothing has stressed
them yet. None of it stops the system from booting, installing, and running — but
knowing where the seams are makes the codebase easier to understand and to improve.

## ACPI without an AML interpreter

The ACPI implementation reads the static tables — the interrupt model, the power
registers, the DSDT — but it does not include an AML interpreter. The methods that
a full ACPI stack would evaluate are left unimplemented. In practice this means
anything that would normally be done through AML, such as reading the clock or
powering the machine off, is handled through direct hardware port access instead.
This works on the hardware tested, but it's less general than a real AML interpreter
would be, and it's an area where a machine with unusual firmware could behave
differently.

## Scale assumptions

Several parts of the system assume a single instance of things. The storage drivers
target a single namespace or a single attached drive rather than enumerating all of
them. The registry is capped at four kilobytes, which is generous for the settings
it holds today but is a fixed ceiling rather than a growing store. These limits are
fine for the current system and were the right simplifications to reach a working
whole, but they're the first things that would need to grow to support more
elaborate setups.

## The unused process structure

The codebase contains a process structure with full register state and process
management fields that isn't currently used, because Rusty OS runs a single userland
binary rather than loading arbitrary programs from disk. Loading ELF executables at
runtime — a real multi-process model — is the natural next step, and the scaffolding
for it is partly in place, but it isn't wired up yet.

## Roadmap

Beyond fixing the issues above, the clear next direction is networking: implementing
network drivers and the ability to fetch data from internet addresses, which would
open up an entire category of new applications. A runtime ELF loader would turn the
single-userland model into a true multi-process system. And the kernel is
deliberately structured so that a completely different userland could be written on
top of it — the syscall boundary is clean enough that the desktop is just one
possible client of the kernel underneath.

Contributions and suggestions toward any of this are welcome. The point of writing
it all from scratch was to understand every layer; the point of documenting it is so
that someone else can too.