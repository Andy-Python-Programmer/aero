# Load the GDT using the `lgdt` instruction.
lgdt [{}]

# Reload all of the segment registers with `null`
mov ax, 0
mov ss, ax
mov ds, ax
mov es, ax
mov fs, ax
mov gs, ax