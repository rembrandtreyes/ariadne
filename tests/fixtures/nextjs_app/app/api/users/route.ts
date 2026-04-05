import { formatDate } from '../../../lib/utils';

export function GET() {
    return { users: [], date: formatDate(new Date()) };
}

export function POST() {
    return { created: true };
}
