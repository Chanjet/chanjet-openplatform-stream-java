package proxy

import (
	"cjtCli/internal/auth"
	"cjtCli/internal/core/config"
	"cjtCli/internal/core/security"
	"cjtCli/internal/core/telemetry"
	"fmt"
	"net"
	"net/http"
	"net/http/httputil"
	"net/url"
	"time"

	"go.uber.org/zap"
)

type ProxyServer interface {
	Start(profile string, cfg *config.Config, port int) error
	Stop() error
}

type loopbackProxy struct {
	tel    *telemetry.Telemetry
	auth   auth.Client
	fw     security.Firewall
	server *http.Server
}

func NewProxyServer(tel *telemetry.Telemetry, auth auth.Client, fw security.Firewall) ProxyServer {
	return &loopbackProxy{
		tel:  tel,
		auth: auth,
		fw:   fw,
	}
}

func (p *loopbackProxy) Start(profile string, cfg *config.Config, port int) error {
	target, err := url.Parse(cfg.OpenApiURL)
	if err != nil {
		return err
	}

	proxy := httputil.NewSingleHostReverseProxy(target)
	
	// Configure TLS Firewall
	proxy.Transport = &http.Transport{
		TLSClientConfig: p.fw.GetTLSConfig(),
		DialContext: (&net.Dialer{
			Timeout:   30 * time.Second,
			KeepAlive: 30 * time.Second,
		}).DialContext,
	}
	
	originalDirector := proxy.Director
	proxy.Director = func(req *http.Request) {
		originalDirector(req)
		
		// Inject Auth
		token, err := p.auth.GetAppAccessToken(profile, cfg)
		if err != nil {
			p.tel.Sys().Error("Proxy failed to get AppAccessToken", zap.Error(err))
			return
		}
		
		req.Header.Set("Authorization", "Bearer "+token.Value)
		// Host must be the target host for many APIs to work correctly
		req.Host = target.Host
		
		p.tel.Audit().Info("Proxying request", 
			zap.String("method", req.Method), 
			zap.String("path", req.URL.Path))
	}

	mux := http.NewServeMux()
	mux.Handle("/", proxy)

	addr := fmt.Sprintf("127.0.0.1:%d", port)
	p.server = &http.Server{
		Addr:    addr,
		Handler: mux,
	}

	// Use a listener to ensure we bind only to 127.0.0.1
	ln, err := net.Listen("tcp", addr)
	if err != nil {
		return err
	}

	p.tel.Sys().Info("Local Loopback Proxy starting", zap.String("addr", addr))
	
	go func() {
		if err := p.server.Serve(ln); err != nil && err != http.ErrServerClosed {
			p.tel.Sys().Error("Proxy server failed", zap.Error(err))
		}
	}()

	return nil
}

func (p *loopbackProxy) Stop() error {
	if p.server != nil {
		p.tel.Sys().Info("Stopping Proxy Server")
		return p.server.Close()
	}
	return nil
}
