# pdf_service

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust Version](https://img.shields.io/badge/rust-1.83%2B-orange.svg)

Servicio en Rust (Actix Web) para generación de PDFs y envío de emails con gestión asíncrona de operaciones.

## 🌟 Características Principales

- **Generación de PDFs**:

  - Conversión de HTML a PDF usando chromiumoxide
  - Soporte para diferentes tamaños de papel y orientaciones
  - Configuración de márgenes personalizada
  - Modo headless optimizado

- **Gestión de Emails**:

  - Envío de correos vía SMTP
  - Soporte para envío asíncrono
  - Seguimiento de estado de envío
  - Reintentos automáticos configurables

- **Sistema de Operaciones**:
  - Gestión de estados (pending/running/done/failed)
  - Almacenamiento en SQLite
  - API REST completa
  - Paginación y filtrado

## 📋 Prerequisitos

### Chromium/Chrome:

- El servicio usa chromiumoxide, el cual lanza Chrome/Chromium en modo headless
- Instalación:
  <details>
  <summary>Linux (Debian/Ubuntu)</summary>

  ```bash
  sudo apt-get update
  sudo apt-get install chromium-browser
  # o en algunos sistemas:
  sudo apt-get install chromium
  ```

  </details>

  <details>
  <summary>Linux (Fedora)</summary>

  ```bash
  sudo dnf install chromium chromium-headless chromedriver
  ```

  </details>

  <details>
  <summary>Linux (CentOS 7)</summary>

  ```bash
  # Habilitar EPEL repository
  sudo yum install epel-release

  # Instalar Chromium
  sudo yum install chromium chromium-headless chromedriver
  ```

  </details>

  <details>
  <summary>Linux (CentOS 8/Rocky Linux/AlmaLinux)</summary>

  ```bash
  # Habilitar EPEL repository
  sudo dnf install epel-release

  # Instalar Chromium
  sudo dnf install chromium chromium-headless chromedriver
  ```

  </details>

  <details>
  <summary>Linux (Arch/Manjaro)</summary>

  ```bash
  sudo pacman -S chromium
  ```

  </details>

  <details>
  <summary>Linux (openSUSE)</summary>

  ```bash
  sudo zypper install chromium chromium-headless
  ```

  </details>

  <details>
  <summary>macOS</summary>

  ```bash
  brew install chromium
  ```

  </details>

### SQLite:

- Base de datos para registro de operaciones y emails
- Archivo por defecto: `./data/operations.db`
- Permisos necesarios de lectura/escritura en `./data/`

### Rust:

- Versión 1.60+ (recomendado 1.83.0)
- Componentes: cargo, rustc
- (Opcional) dotenv para variables de entorno

## 🚀 Inicio Rápido

### Instalación Local

1. **Clonar el repositorio**:

```bash
git clone https://tu-repo.git
cd pdf_service
```

2. **Instalar dependencias**:

<details>
<summary>Linux (Debian/Ubuntu)</summary>

```bash
sudo apt-get update
sudo apt-get install -y \
    libssl-dev \
    pkg-config \
    chromium-browser \
    sqlite3 \
    build-essential \
    curl \
    gcc \
    make
```

</details>

<details>
<summary>Linux (Fedora)</summary>

```bash
sudo dnf install \
    openssl-devel \
    pkg-config \
    chromium \
    sqlite \
    sqlite-devel \
    gcc \
    make \
    curl \
    perl-core
```

</details>

<details>
<summary>Linux (CentOS 7)</summary>

```bash
# Habilitar EPEL
sudo yum install epel-release

# Instalar dependencias
sudo yum install \
    openssl-devel \
    pkg-config \
    chromium \
    sqlite \
    sqlite-devel \
    gcc \
    make \
    curl \
    perl-core
```

</details>

<details>
<summary>Linux (CentOS 8/Rocky Linux/AlmaLinux)</summary>

```bash
# Habilitar EPEL
sudo dnf install epel-release

# Instalar dependencias
sudo dnf install \
    openssl-devel \
    pkg-config \
    chromium \
    sqlite \
    sqlite-devel \
    gcc \
    make \
    curl \
    perl-core
```

</details>

<details>
<summary>Linux (Arch/Manjaro)</summary>

```bash
sudo pacman -S \
    openssl \
    pkg-config \
    chromium \
    sqlite \
    base-devel \
    curl
```

</details>

<details>
<summary>Linux (openSUSE)</summary>

```bash
sudo zypper install \
    libopenssl-devel \
    pkg-config \
    chromium \
    sqlite3 \
    sqlite3-devel \
    gcc \
    make \
    curl
```

</details>

<details>
<summary>macOS (Homebrew)</summary>

```bash
brew install \
    openssl \
    pkg-config \
    chromium \
    sqlite
```

</details>

3. **Compilar y Ejecutar**:

```bash
cargo build --release
./target/release/pdf_service
```

### 🐳 Usando Docker

```bash
# Construir imagen
docker build -t pdf-service .

# Ejecutar contenedor
docker run -p 5022:5022 \
    -v $(pwd)/data:/usr/src/app/data \
    -v $(pwd)/migrations:/usr/src/app/migrations \
    pdf-service
```

## 📖 Documentación de la API

### Generación de PDFs

#### `POST /api/pdf`

Genera un PDF a partir de HTML.

**Request**:

```json
{
  "html": "<h1>Hola mundo</h1><p>Contenido del PDF</p>",
  "orientation": "portrait",
  "paper_size": {
    "width": 8.5,
    "height": 11.0
  },
  "margins": {
    "top": 0.5,
    "bottom": 0.5,
    "left": 0.5,
    "right": 0.5
  },
  "size_category": "small"
}
```

**Response**: Binary PDF file

### Envío de Emails

#### `POST /api/email/send`

Envía un email, opcionalmente de forma asíncrona.

**Request**:

```json
{
  "smtp_host": "smtp.gmail.com",
  "smtp_port": 587,
  "smtp_user": "your-email@gmail.com",
  "smtp_pass": "your-app-password",
  "recipient": "destination@example.com",
  "subject": "Test Email",
  "body": "Email content",
  "async_send": true
}
```

**Response**:

```json
{
  "operation_id": "abc123...",
  "message": "Email queued for delivery"
}
```

### Gestión de Operaciones

#### `GET /api/operations`

Lista todas las operaciones con paginación.

**Query Parameters**:

- `page`: Número de página (default: 1)
- `page_size`: Elementos por página (default: 10)

#### `GET /api/operations/:id`

Obtiene detalles de una operación específica.

## 🔧 Configuración

### Variables de Entorno

Crea un archivo `.env` en la raíz del proyecto:

```env
# Server Configuration
SERVER_HOST=127.0.0.1
SERVER_PORT=5022

# Email Configuration
SMTP_DEFAULT_HOST=smtp.gmail.com
SMTP_DEFAULT_PORT=587

# Database Configuration
DATABASE_PATH=./data/operations.db

# PDF Generation
CHROME_PATH=/usr/bin/chromium
```

### Systemd Service

Para sistemas Linux, ejemplo de configuración systemd:

```ini
[Unit]
Description=PDF Service in Rust
After=network.target

[Service]
ExecStart=/usr/local/bin/pdf_service
WorkingDirectory=/usr/local/bin
Restart=always
User=www-data
Group=www-data
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

## 📁 Estructura del Proyecto

```
pdf_service/
├── src/
│   ├── main.rs                 # Punto de entrada
│   ├── config/                 # Configuración
│   ├── services/              # Lógica de negocio
│   ├── handlers/              # Endpoints HTTP
│   ├── models/                # Modelos de datos
│   └── tests/                 # Tests
├── migrations/                # SQL migrations
├── Dockerfile                # Configuración Docker
├── .env.example             # Template variables de entorno
└── README.md                # Este archivo
```

## 🧪 Tests

```bash
# Ejecutar todos los tests
cargo test

# Ejecutar tests con logs
RUST_LOG=debug cargo test

# Tests específicos
cargo test email_service
```

## 🤝 Contribuir

1. Fork el proyecto
2. Crea tu Feature Branch (`git checkout -b feature/AmazingFeature`)
3. Commit tus cambios (`git commit -m 'Add some AmazingFeature'`)
4. Push al Branch (`git push origin feature/AmazingFeature`)
5. Abre un Pull Request

## 📝 Changelog

### [1.0.0] - 2024-01-09

- Implementación inicial
- Soporte para generación de PDFs
- Sistema de envío de emails
- API REST completa

## 📜 Licencia

Este proyecto está bajo la Licencia MIT - ver el archivo [LICENSE](LICENSE) para detalles.

## ✨ Agradecimientos

- [chromiumoxide](https://github.com/mattsse/chromiumoxide)
- [actix-web](https://github.com/actix/actix-web)
- [sqlx](https://github.com/launchbadge/sqlx)
- [lettre](https://github.com/lettre/lettre)
