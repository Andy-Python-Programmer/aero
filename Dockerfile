FROM ubuntu:latest 
WORKDIR /opt/workdir
# Cargo executables are installed here:
ENV PATH="$PATH:/home/workuser/.cargo/bin"
# Python executables are installed here:
ENV PATH="$PATH:/home/workuser/.local/bin"

RUN apt-get update
RUN apt-get install -y \
    autopoint \
    bash \
    binutils \
    bison \
    cmake \
    coreutils \
    curl \
    expat \
    flex \
    gcc \
    gettext \
    git \
    gperf \
    groff \
    gzip \
    help2man \
    libgmp-dev \
    m4 \
    make \
    mercurial \
    meson \
    mtools \
    nasm \
    openssl \
    patch \
    perl \
    python3 \
    python3-mako \
    python3-pip \
    rsync \
    subversion \
    tar \
    texinfo \
    wget \
    xcb-proto \
    xorriso \
    xsltproc \
    xz-utils
RUN useradd -m workuser

USER workuser
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
RUN python3 -m pip install requests xbstrap
ENTRYPOINT python3 aero.py --no-run