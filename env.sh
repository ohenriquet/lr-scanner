#!/usr/bin/env bash
# Carrega o ambiente de desenvolvimento do projeto (toolchains portáteis em C:\dev).
export CARGO_HOME="$USERPROFILE/.cargo"
export RUSTUP_HOME="$USERPROFILE/.rustup"
export JAVA_HOME="/c/dev/jdk/jdk-21.0.13+9"
export FLUTTER_ROOT="/c/dev/flutter"
export GHIDRA_HOME="/c/dev/ghidra/ghidra_12.2_PUBLIC"
export PATH="$CARGO_HOME/bin:/c/dev/mingw64/bin:$FLUTTER_ROOT/bin:$JAVA_HOME/bin:$PATH"

# Android (SDK + NDK portáteis em C:\dev\android)
export ANDROID_HOME="/c/dev/android"
export ANDROID_SDK_ROOT="$ANDROID_HOME"
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/27.2.12479018"
export PATH="$PATH:$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin"
