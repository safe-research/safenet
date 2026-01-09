import Link from "@docusaurus/Link";
import Heading from "@theme/Heading";
import Layout from "@theme/Layout";
import styles from "./index.module.css";

function HomepageHeader() {
	return (
		<header className={styles.heroBanner}>
			<div className="container">
				<Heading as="h1" className="hero__title">
					Shieldnet
				</Heading>
				<p className="hero__subtitle">
					Shieldnet enforces transaction security onchain - protecting users from high-risk threats
				</p>
				<Link className="button button--secondary button--lg" to="/docs/introduction">
					Get Started
				</Link>
			</div>
		</header>
	);
}

export default function Home() {
	return (
		<Layout title="Shieldnet" description="Shieldnet enforces transaction security onchain">
			<HomepageHeader />
			<main>{/* TODO */}</main>
		</Layout>
	);
}
