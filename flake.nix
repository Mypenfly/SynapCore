# flake.nix
{
  description = "SynapCore - Multi-agent AI orchestration system with Rust core, Python agents, Dart frontend, and Nushell ops";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            # 核心技术栈
            # rust
            rustc
            cargo
            clippy
            rustfmt
            rust-analyzer
            cargo-watch # 热重载开发
            cargo-edit # 依赖管理 (cargo add/rm)
            cargo-outdated # 检查过期依赖
            cargo-audit # 安全检查

            #python
            python3
            uv

            #dart
            dart
            flutter

            # 运维工具链
            nushell
            git
            sqlite
            jq
            watchexec

          ];
          nativeBuildInputs = with pkgs; [
            pkg-config
            openssl
            openssl.dev
          ];

          shellHook = ''
            # 项目横幅
            echo ""
            echo "   ███████╗██╗   ██╗███╗   ██╗ █████╗ ██████╗ ██████╗ ██████╗ ██████╗ ███████╗"
            echo "   ██╔════╝╚██╗ ██╔╝████╗  ██║██╔══██╗██╔══██╗██╔══██╗██╔══██╗██╔══██╗██╔════╝"
            echo "   ███████╗ ╚████╔╝ ██╔██╗ ██║███████║██████╔╝██████╔╝██████╔╝██████╔╝█████╗  "
            echo "   ╚════██║  ╚██╔╝  ██║╚██╗██║██╔══██║██╔═══╝ ██╔═══╝ ██╔══██╗██╔══██╗██╔══╝  "
            echo "   ███████║   ██║   ██║ ╚████║██║  ██║██║     ██║     ██║  ██║██████╔╝███████╗"
            echo "   ╚══════╝   ╚═╝   ╚═╝  ╚═══╝╚═╝  ╚═╝╚═╝     ╚═╝     ╚═╝  ╚═╝╚═════╝ ╚══════╝"
            echo ""
            echo "              Rust Core × Python Agents × Flutter UI × Nushell Ops"
            echo ""

            # 环境信息
            echo "📊 技术栈版本"
            echo "   Rust:      $(rustc --version | cut -d' ' -f2)"
            echo "   Python:    $(python3 --version | cut -d' ' -f2)"
            echo "   Dart:      $(dart --version)"
            echo "   Nushell:   $(nu --version 2>/dev/null | head -1 | cut -d' ' -f2 || echo '等待启动')"
            echo ""

            # 路径配置
            export PNPM_HOME="./.pnpm-store"
            mkdir -p "$PNPM_HOME" 2>/dev/null || true
            export RUST_BACKTRACE=1
            export RUST_LOG=info

            echo "🚪 进入Nushell工作流..."
            echo "   项目脚本位于 scripts/ 目录"
            echo "   使用 nu脚本名>.nu 执行"
            echo ""
            echo "-----------------------------------------------"

            # 启动Nushell
            exec nu --no-config-file --no-history
          '';
        };
      }
    );
}
