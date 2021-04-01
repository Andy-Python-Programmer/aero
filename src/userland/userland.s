.intel_syntax noprefix

cli

# Set the ds, es, fs and gs segment selectors to our user mode data selector - 0x23
mov ds, ax
mov es, ax  
mov fs, ax 
mov gs, ax
mov rsi, rsp

push rax
push rsi
push 0x200
push rdx
push rdi

iretq