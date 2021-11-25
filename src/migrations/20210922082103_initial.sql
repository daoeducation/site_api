CREATE TYPE payment_method AS ENUM (
  'stripe',
  'btcpay'
);

CREATE TYPE PlanCode AS ENUM (
  'Global',
  'Europe',
  'Latam',
  'Guest'
);

CREATE TABLE students (
  id SERIAL PRIMARY KEY NOT NULL,
  email VARCHAR NOT NULL,
  full_name VARCHAR NOT NULL,
  country VARCHAR NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  phone VARCHAR,
  tax_number VARCHAR,
  tax_address VARCHAR,
  referral_code VARCHAR,
  current_subscription_id INTEGER,
  wordpress_user VARCHAR,
  wordpress_initial_password VARCHAR,
  discord_user_id VARCHAR,
  discord_handle VARCHAR,
  discord_verification VARCHAR,
  stripe_customer_id VARCHAR,
  payment_method payment_method NOT NULL DEFAULT 'stripe'
);

CREATE INDEX student_subscription ON students (current_subscription_id);
CREATE INDEX student_email ON students (email);

CREATE TABLE subscriptions (
  id SERIAL PRIMARY KEY NOT NULL,
  created_at TIMESTAMPTZ NOT NULL,
  invoicing_day INTEGER NOT NULL,
  student_id INTEGER NOT NULL,
  active BOOLEAN NOT NULL DEFAULT FALSE, -- Should we keep charging this every month?
  plan_code VARCHAR NOT NULL, -- Full, 50% off, 25% off, Free.
  price DECIMAL NOT NULL,
  paid BOOLEAN NOT NULL DEFAULT FALSE,
  paid_at TIMESTAMPTZ,
  stripe_subscription_id VARCHAR
);

CREATE INDEX subscription_paid ON subscriptions (paid);
CREATE INDEX subscription_student_id ON subscriptions (student_id);
CREATE INDEX subscription_active ON subscriptions (active);

CREATE TABLE monthly_charges (
  id SERIAL PRIMARY KEY NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  billing_period TIMESTAMPTZ NOT NULL,
  subscription_id INTEGER NOT NULL,
  student_id INTEGER NOT NULL,
  price DECIMAL NOT NULL,
  paid BOOLEAN NOT NULL DEFAULT FALSE,
  paid_at TIMESTAMPTZ
);
CREATE INDEX monthly_charge_paid ON monthly_charges (paid);
CREATE INDEX monthly_charge_subscription_id ON monthly_charges (subscription_id);

CREATE TABLE degrees (
  id SERIAL PRIMARY KEY NOT NULL,
  subscription_id INTEGER NOT NULL,
  student_id INTEGER NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  description VARCHAR NOT NULL,
  poap_link VARCHAR,
  constata_certificate_id VARCHAR,
  price DECIMAL NOT NULL,
  paid BOOLEAN NOT NULL DEFAULT FALSE,
  paid_at TIMESTAMPTZ
);

CREATE INDEX degrees_paid ON degrees (paid);
CREATE INDEX degrees_subscription_id ON degrees (subscription_id);

CREATE TABLE payments (
  id SERIAL PRIMARY KEY NOT NULL,
  student_id INTEGER NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  amount DECIMAL NOT NULL,
  fees DECIMAL NOT NULL,
  payment_method payment_method NOT NULL,
  clearing_data TEXT NOT NULL,
  invoice_id INTEGER
);
CREATE INDEX payments_student_id ON payments (student_id);
CREATE INDEX payments_invoice_id ON payments (invoice_id);

CREATE TABLE invoices (
  id SERIAL PRIMARY KEY NOT NULL,
  student_id INTEGER NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  amount DECIMAL NOT NULL,
  payment_method payment_method NOT NULL,
  description TEXT NOT NULL,
  external_id VARCHAR NOT NULL,
  url TEXT NOT NULL,
  notified_on TIMESTAMPTZ,
  paid BOOLEAN NOT NULL DEFAULT FALSE,
  payment_id INTEGER,
  expired BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE INDEX invoices_student_id ON invoices (student_id);
CREATE INDEX invoices_payment_id ON invoices (payment_id);
