package main

import (
	"context"
	"log/slog"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	"github.com/jackc/pgx/v5/pgxpool"

	"github.com/aether-engine/identity/internal/config"
	"github.com/aether-engine/identity/internal/handler"
	"github.com/aether-engine/identity/internal/model"
	"github.com/aether-engine/identity/internal/repository"
	"github.com/aether-engine/identity/internal/service"
)

const (
	ReadHeaderTimeout = 10 * time.Second
	ReadTimeout       = 30 * time.Second
	WriteTimeout      = 30 * time.Second
	IdleTimeout       = 120 * time.Second
	ShutdownTimeout   = 30 * time.Second
)

func main() {
	logger := slog.New(slog.NewJSONHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelInfo}))
	slog.SetDefault(logger)

	cfg, err := config.Load()
	if err != nil {
		logger.Error("failed to load config", "error", err)
		os.Exit(1)
	}

	// Database connection
	dbPool, err := pgxpool.New(context.Background(), cfg.DatabaseURL)
	if err != nil {
		logger.Error("failed to connect to database", "error", err)
		os.Exit(1)
	}
	defer dbPool.Close()

	if err := dbPool.Ping(context.Background()); err != nil {
		logger.Error("failed to ping database", "error", err)
		os.Exit(1)
	}
	logger.Info("database connected")

	// Read replicas for scale-out reads
	readReplicas := make([]*pgxpool.Pool, 0, len(cfg.DatabaseReadReplicas))
	for _, replicaURL := range cfg.DatabaseReadReplicas {
		replicaPool, err := pgxpool.New(context.Background(), replicaURL)
		if err != nil {
			logger.Error("failed to connect to read replica", "error", err, "url", replicaURL)
			os.Exit(1)
		}
		readReplicas = append(readReplicas, replicaPool)
	}
	defer func() {
		for _, replica := range readReplicas {
			replica.Close()
		}
	}()

	// Repositories
	userRepo := repository.NewUserRepositoryWithReadReplicas(dbPool, readReplicas)
	sessionRepo := repository.NewSessionRepositoryWithReadReplicas(dbPool, readReplicas)
	auditRepo := repository.NewAuditRepositoryWithReadReplicas(dbPool, readReplicas)
	oauthAccountRepo := repository.NewOAuthAccountRepositoryWithReadReplicas(dbPool, readReplicas)
	webauthnCredRepo := repository.NewWebAuthnCredentialRepositoryWithReadReplicas(dbPool, readReplicas)

	// Services
	authService, err := service.NewAuthService(
		cfg,
		userRepo,
		sessionRepo,
		oauthAccountRepo,
		webauthnCredRepo,
		auditRepo,
		logger,
	)
	if err != nil {
		logger.Error("failed to create auth service", "error", err)
		os.Exit(1)
	}
	profileService := service.NewProfileService(userRepo, auditRepo, logger)
	permissionService := service.NewPermissionService(userRepo, auditRepo, logger)

	// Handlers
	authHandler := handler.NewAuthHandler(authService, logger)
	profileHandler := handler.NewProfileHandler(profileService, logger)
	tokenHandler := handler.NewTokenHandler(authService, logger)
	permissionHandler := handler.NewPermissionHandler(permissionService, logger)
	oauthHandler := handler.NewOAuthHandler(authService, logger)
	webauthnHandler := handler.NewWebAuthnHandler(authService, logger)

	// Router
	r := chi.NewRouter()
	r.Use(middleware.Recoverer)
	r.Use(middleware.RequestID)
	r.Use(middleware.RealIP)
	r.Use(handler.RequestLogger(logger))

	// Health check
	r.Get("/health", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"status":"ok"}`))
	})

	// Public routes
	r.Route("/api/v1", func(r chi.Router) {
		// Auth (no auth required)
		r.Post("/auth/register", authHandler.Register)
		r.Post("/auth/login", authHandler.Login)
		r.Post("/auth/refresh", authHandler.Refresh)
		r.Post("/auth/oauth/{provider}/login", oauthHandler.Login)
		r.Post("/auth/webauthn/login", webauthnHandler.Login)

		// Token validation (for world servers)
		r.Post("/auth/validate", tokenHandler.Validate)
		r.Get("/auth/.well-known/jwks.json", tokenHandler.JWKS)

		// Authenticated routes
		r.Group(func(r chi.Router) {
			r.Use(handler.AuthMiddleware(authService))

			r.Post("/auth/logout", authHandler.Logout)

			// Profile
			r.Get("/profiles/me", profileHandler.GetMe)
			r.Put("/profiles/me", profileHandler.UpdateMe)
			r.Get("/profiles/{id}", profileHandler.GetByID)
			r.Get("/profiles", profileHandler.Search)
			r.Post("/auth/oauth/{provider}/link", oauthHandler.Link)
			r.Post("/auth/webauthn/register", webauthnHandler.Register)

			// Permissions
			r.Get("/permissions/me", permissionHandler.GetMyPermissions)

			// Admin routes
			r.Group(func(r chi.Router) {
				r.Use(handler.RequirePermission(model.PermRoleAssign))
				r.Post("/admin/roles/{user_id}", permissionHandler.AssignRole)
			})
			r.Group(func(r chi.Router) {
				r.Use(handler.RequirePermission(model.PermRoleRevoke))
				r.Delete("/admin/roles/{user_id}/{role}", permissionHandler.RevokeRole)
			})
		})
	})

	// Server
	srv := &http.Server{
		Addr:              ":" + cfg.Port,
		Handler:           r,
		ReadHeaderTimeout: ReadHeaderTimeout,
		ReadTimeout:       ReadTimeout,
		WriteTimeout:      WriteTimeout,
		IdleTimeout:       IdleTimeout,
	}

	// Graceful shutdown
	go func() {
		logger.Info("server starting", "port", cfg.Port)
		if err := srv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			logger.Error("server failed", "error", err)
			os.Exit(1)
		}
	}()

	quit := make(chan os.Signal, 1)
	signal.Notify(quit, syscall.SIGINT, syscall.SIGTERM)
	<-quit

	logger.Info("server shutting down")
	ctx, cancel := context.WithTimeout(context.Background(), ShutdownTimeout)
	defer cancel()

	if err := srv.Shutdown(ctx); err != nil {
		logger.Error("server forced to shutdown", "error", err)
		os.Exit(1)
	}
	logger.Info("server stopped")
}
