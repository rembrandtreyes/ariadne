import { formatDate } from '../lib/utils';

export default function Page() {
    return formatDate(new Date());
}

export function generateMetadata() {
    return { title: 'Home' };
}
