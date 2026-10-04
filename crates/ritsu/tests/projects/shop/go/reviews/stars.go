// Reviews keep to themselves: nothing goes between reviews and billing.
package reviews

func Average(stars []int) float64 {
	if len(stars) == 0 {
		return 0
	}
	sum := 0
	for _, s := range stars {
		sum += s
	}
	return float64(sum) / float64(len(stars))
}
