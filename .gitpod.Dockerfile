FROM gitpod/workspace-full-vnc

RUN sudo apt-get install -y \
    nasm \
    cpio \
    xorriso \
    qemu-system-x86-64
    
RUN python3 -m pip install requests xbstrap
