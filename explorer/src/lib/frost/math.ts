import type { WeierstrassPoint } from "@noble/curves/abstract/weierstrass.js";
import { secp256k1 } from "@noble/curves/secp256k1.js";

export type FrostPoint = WeierstrassPoint<bigint>;

export const G_BASE = secp256k1.Point.BASE;
export const N = secp256k1.Point.Fn.ORDER;

export const neg = (val: FrostPoint): FrostPoint => val.negate();
export const add = (lhs: FrostPoint, rhs: FrostPoint): FrostPoint => lhs.add(rhs);
export const sub = (lhs: FrostPoint, rhs: FrostPoint): FrostPoint => lhs.subtract(rhs);
export const multiply = (lhs: FrostPoint, rhs: bigint): FrostPoint => lhs.multiply(rhs);
export const g = (scalar: bigint): FrostPoint => secp256k1.Point.BASE.multiply(scalar);

export const toPoint = (coordinates: { x: bigint; y: bigint }): FrostPoint => {
	const point = secp256k1.Point.fromAffine(coordinates);
	point.assertValidity();
	return point;
};
