{
  pkgs ? import <nixpkgs> { },
  lib ? pkgs.lib,
  stdenv ? pkgs.stdenv,
  fetchurl ? pkgs.fetchurl,
  makeWrapper ? pkgs.makeWrapper,
  jre ? pkgs.jre,
}:
let
  rust-overlay = (
    import (builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz")
  );
  zipkin = stdenv.mkDerivation rec {
    version = "3.2.1";
    pname = "zipkin-server";
    src = fetchurl {
      url = "https://search.maven.org/remote_content?g=io.zipkin&a=zipkin-server&v=${version}&c=exec";
      sha256 = "GZll+hknrWpBN35WXNUwvdUaw53fA/cRtgYoaBdXwbQ=";
    };
    nativeBuildInputs = [ makeWrapper ];

    buildCommand = ''
      mkdir -p $out/share/java
      cp ${src} $out/share/java/zipkin-server-${version}-exec.jar
      mkdir -p $out/bin
      makeWrapper ${jre}/bin/java $out/bin/zipkin-server \
        --add-flags "-jar $out/share/java/zipkin-server-${version}-exec.jar"
    '';
    meta = with lib; {
      description = "Zipkin distributed tracing system";
      homepage = "https://zipkin.io/";
      sourceProvenance = with sourceTypes; [ binaryBytecode ];
      license = licenses.asl20;
      platforms = platforms.unix;
      maintainers = [ maintainers.hectorj ];
      mainProgram = "zipkin-server";
    };
  };
  pkgs = (import <nixpkgs> { overlays = [ rust-overlay ]; });
in
pkgs.mkShell {
  packages = [
    pkgs.openssl
    pkgs.nixfmt-rfc-style
    pkgs.pkg-config
    pkgs.protobuf
    (pkgs.rust-bin.beta.latest.default.override {
      extensions = [
        "rust-src"
        "rust-analyzer"
      ];
    })
    zipkin
  ];
}
