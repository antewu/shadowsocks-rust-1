//! TCP relay client implementation

use std::{
    io,
    net::SocketAddr,
    pin::Pin,
    task::{self, Poll},
};

use log::trace;
use tokio::{net::TcpStream, prelude::*};

use crate::relay::socks5::{
    self,
    Address,
    Command,
    HandshakeRequest,
    HandshakeResponse,
    Reply,
    TcpRequestHeader,
    TcpResponseHeader,
};

use super::ProxyStream;
use crate::{config::ServerConfig, context::SharedContext};

/// Socks5 proxy client
pub struct Socks5Client {
    stream: TcpStream,
}

impl Socks5Client {
    /// Connects to `addr` via `proxy`
    pub async fn connect<A>(addr: A, proxy: &SocketAddr) -> io::Result<Socks5Client>
    where
        Address: From<A>,
    {
        let mut s = TcpStream::connect(proxy).await?;

        // 1. Handshake
        let hs = HandshakeRequest::new(vec![socks5::SOCKS5_AUTH_METHOD_NONE]);
        trace!("client connected, going to send handshake: {:?}", hs);

        hs.write_to(&mut s).await?;

        let hsp = HandshakeResponse::read_from(&mut s).await?;

        trace!("got handshake response: {:?}", hsp);
        assert_eq!(hsp.chosen_method, socks5::SOCKS5_AUTH_METHOD_NONE);

        // 2. Send request header
        let h = TcpRequestHeader::new(Command::TcpConnect, From::from(addr));
        trace!("going to connect, req: {:?}", h);
        h.write_to(&mut s).await?;

        let hp = TcpResponseHeader::read_from(&mut s).await?;

        trace!("got response: {:?}", hp);
        match hp.reply {
            Reply::Succeeded => (),
            r => {
                let err = io::Error::new(io::ErrorKind::Other, format!("{}", r));
                return Err(err);
            }
        }

        Ok(Socks5Client { stream: s })
    }

    /// UDP Associate `addr` via `proxy`
    pub async fn udp_associate<A>(addr: A, proxy: &SocketAddr) -> io::Result<(Socks5Client, Address)>
    where
        Address: From<A>,
    {
        let mut s = TcpStream::connect(proxy).await?;

        // 1. Handshake
        let hs = HandshakeRequest::new(vec![socks5::SOCKS5_AUTH_METHOD_NONE]);
        trace!("client connected, going to send handshake: {:?}", hs);

        hs.write_to(&mut s).await?;
        s.flush().await?;

        let hsp = HandshakeResponse::read_from(&mut s).await?;

        trace!("got handshake response: {:?}", hsp);
        assert_eq!(hsp.chosen_method, socks5::SOCKS5_AUTH_METHOD_NONE);

        // 2. Send request header
        let h = TcpRequestHeader::new(Command::UdpAssociate, From::from(addr));
        trace!("going to connect, req: {:?}", h);

        h.write_to(&mut s).await?;
        s.flush().await?;
        let hp = TcpResponseHeader::read_from(&mut s).await?;

        trace!("got response: {:?}", hp);
        match hp.reply {
            Reply::Succeeded => (),
            r => {
                let err = io::Error::new(io::ErrorKind::Other, format!("{}", r));
                return Err(err);
            }
        }

        Ok((Socks5Client { stream: s }, hp.address))
    }
}

impl AsyncRead for Socks5Client {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut task::Context, buf: &mut [u8]) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for Socks5Client {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut task::Context, buf: &[u8]) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

/// Shadowsocks' TCP client
pub struct ServerClient {
    stream: ProxyStream,
}

impl ServerClient {
    /// Connect to target address via shadowsocks' server
    pub async fn connect(context: SharedContext, addr: &Address, svr_cfg: &ServerConfig) -> io::Result<ServerClient> {
        let stream = ProxyStream::connect_proxied(context, svr_cfg, addr).await?;
        Ok(ServerClient { stream })
    }
}

impl AsyncRead for ServerClient {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut task::Context, buf: &mut [u8]) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for ServerClient {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut task::Context, buf: &[u8]) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}
