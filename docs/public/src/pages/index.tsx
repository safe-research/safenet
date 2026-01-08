import React from 'react';
import clsx from 'clsx';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';
import styles from './index.module.css';

function HomepageHeader() {
  return (
    <header className={clsx('hero hero--primary', styles.heroBanner)}>
      <div className="container">
        <Heading as="h1" className="hero__title">
          Shieldnet
        </Heading>
        <p className="hero__subtitle">
          Shieldnet enforces transaction security onchain - protecting users from high-risk threats
        </p>
        <div className={styles.buttons}>
          <a
            className="button button--secondary button--lg"
            href="/docs/introduction">
            Get Started
          </a>
        </div>
      </div>
    </header>
  );
}

export default function Home(): JSX.Element {
  return (
    <Layout
      title="Shieldnet"
      description="Shieldnet enforces transaction security onchain">
      <HomepageHeader />
      <main>
        {/* TODO */}
      </main>
    </Layout>
  );
}